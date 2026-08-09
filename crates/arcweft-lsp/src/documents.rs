use crate::positions::{LineIndex, PositionEncoding};
use crate::uri_key::LspUriKey;
use arcweft_lang_syntax::{
    incremental::{ParseFailure, ParsedSource, SyntaxDatabase, SyntaxDatabaseCreateError},
    parser::ParseOptions,
};
use arcweft_source::{
    SourceDocument, SourceDocumentError, SourceDocumentId, SourceDocumentIdError, SourceEdit,
    SourceName, SourceRange, identity::SourceSnapshotId,
};
use lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, Uri};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

/// One open text document snapshot.
#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    uri: Uri,
    version: i32,
    parsed: ParsedSource,
    line_index: LineIndex,
}

/// Open document cache and sole incremental syntax session for FULL synchronization.
#[derive(Debug)]
pub struct DocumentStore {
    syntax: SyntaxDatabase,
    documents: BTreeMap<LspUriKey, DocumentSnapshot>,
}

/// One accepted compiler parse prepared for atomic adoption by the live store.
#[derive(Clone, Debug)]
pub(crate) struct ParsedSourceAdoption {
    uri: LspUriKey,
    version: i32,
    parsed: ParsedSource,
}

/// Exact accepted source authority used to open one live editor document.
#[derive(Clone, Debug)]
pub(crate) struct AcceptedOpenDocument {
    document: Arc<SourceDocument>,
    parsed: Option<ParsedSource>,
}

/// Fully validated adoption batch. Construction is store-owned so commit is infallible.
#[derive(Debug)]
pub(crate) struct ValidatedParsedSourceAdoptions(Vec<ParsedSourceAdoption>);

/// Document cache update error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DocumentError {
    /// FULL sync requires exactly one full-text content change.
    #[error("Arcweft LSP currently expects exactly one full-text document change")]
    ExpectedFullSyncChange,
    /// A change can only advance an existing open syntax lineage.
    #[error("document `{uri}` is not open")]
    DocumentNotOpen { uri: String },
    /// LSP versions must advance monotonically within one open lineage.
    #[error("document version {supplied} is stale; current version is {current}")]
    StaleVersion { current: i32, supplied: i32 },
    /// The incremental syntax transaction failed without publishing a snapshot.
    #[error(transparent)]
    Syntax(#[from] ParseFailure),
    /// The URI could not form a logical source identity.
    #[error(transparent)]
    SourceId(#[from] SourceDocumentIdError),
    /// The editor bytes could not form an immutable source document.
    #[error(transparent)]
    SourceDocument(#[from] SourceDocumentError),
}

/// An accepted project parse cannot replace the exact live editor authority.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum ParsedSourceAdoptionError {
    #[error("accepted syntax adoption contains duplicate URI `{uri}`")]
    DuplicateUri { uri: String },
    #[error("accepted syntax adoption targets a document that is no longer open: `{uri}`")]
    DocumentNotOpen { uri: String },
    #[error(
        "accepted syntax adoption version changed for `{uri}`: expected {expected}, found {actual}"
    )]
    VersionChanged {
        uri: String,
        expected: i32,
        actual: i32,
    },
    #[error("accepted syntax adoption bytes differ from the live document: `{uri}`")]
    SourceChanged { uri: String },
    #[error("accepted syntax adoption belongs to another syntax session: `{uri}`")]
    ForeignSyntaxSession { uri: String },
    #[error("accepted syntax adoption is not the current committed grammar snapshot: `{uri}`")]
    StaleSyntaxSnapshot { uri: String },
}

impl DocumentSnapshot {
    fn new(uri: Uri, version: i32, parsed: ParsedSource, encoding: PositionEncoding) -> Self {
        let line_index = LineIndex::new(Arc::<str>::from(parsed.source()), encoding);
        Self {
            uri,
            version,
            parsed,
            line_index,
        }
    }

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
        self.parsed.source()
    }

    /// Exact revision-bound source document lease for this open snapshot.
    pub fn source_document(&self) -> &Arc<SourceDocument> {
        self.parsed.document_lease()
    }

    /// Exact attached syntax lease accepted for this open snapshot.
    pub const fn parsed_source(&self) -> &ParsedSource {
        &self.parsed
    }

    /// Source-aware line index for this snapshot.
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }
}

impl ParsedSourceAdoption {
    pub(crate) fn new(uri: LspUriKey, version: i32, parsed: ParsedSource) -> Self {
        Self {
            uri,
            version,
            parsed,
        }
    }
}

impl AcceptedOpenDocument {
    pub(crate) fn new(document: Arc<SourceDocument>, parsed: Option<ParsedSource>) -> Self {
        debug_assert!(parsed.as_ref().is_none_or(|parsed| {
            Arc::ptr_eq(parsed.document_lease(), &document)
                || parsed.document().identity() == document.identity()
        }));
        Self { document, parsed }
    }

    pub(crate) const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub(crate) const fn parsed(&self) -> Option<&ParsedSource> {
        self.parsed.as_ref()
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::try_new().expect("an LSP session can allocate one syntax database identity")
    }
}

impl DocumentStore {
    /// Creates an empty document store with one never-reused syntax session.
    pub fn try_new() -> Result<Self, SyntaxDatabaseCreateError> {
        Ok(Self {
            syntax: SyntaxDatabase::try_new()?,
            documents: BTreeMap::new(),
        })
    }

    /// Parses and publishes a newly opened document as one fresh syntax lineage.
    pub fn open(
        &mut self,
        params: DidOpenTextDocumentParams,
        encoding: PositionEncoding,
    ) -> Result<DocumentSnapshot, DocumentError> {
        self.open_with_authority(params, encoding, None)
    }

    /// Opens through an already accepted project lineage when one exists.
    pub(crate) fn open_with_authority(
        &mut self,
        params: DidOpenTextDocumentParams,
        encoding: PositionEncoding,
        accepted: Option<&AcceptedOpenDocument>,
    ) -> Result<DocumentSnapshot, DocumentError> {
        let text: Arc<str> = Arc::from(params.text_document.text);
        let parsed = if let Some(accepted) = accepted.and_then(AcceptedOpenDocument::parsed) {
            if accepted.source() == text.as_ref() {
                self.syntax
                    .reparse(accepted, &[], ParseOptions::default())?
            } else {
                let whole = accepted
                    .document()
                    .span(SourceRange::new(0, accepted.source().len()))
                    .expect("the accepted whole-document edit uses exact UTF-8 bytes");
                self.syntax.reparse(
                    accepted,
                    &[SourceEdit::new(whole, text.as_ref())],
                    ParseOptions::default(),
                )?
            }
        } else {
            let accepted_document = accepted.map(AcceptedOpenDocument::document);
            let document = if let Some(accepted) = accepted_document {
                if accepted.text() == text.as_ref() {
                    Arc::clone(accepted)
                } else {
                    Arc::new(SourceDocument::try_new(
                        accepted.identity().id().clone(),
                        accepted.display_name().clone(),
                        Arc::clone(&text),
                    )?)
                }
            } else {
                Arc::new(SourceDocument::try_new(
                    SourceDocumentId::try_new(params.text_document.uri.to_string())?,
                    SourceName::path(params.text_document.uri.to_string()),
                    Arc::clone(&text),
                )?)
            };
            self.syntax.parse_initial(
                SourceSnapshotId::initial(document.display_name().clone()),
                document,
                ParseOptions::default(),
            )?
        };
        let snapshot = DocumentSnapshot::new(
            params.text_document.uri,
            params.text_document.version,
            parsed,
            encoding,
        );
        self.documents
            .insert(LspUriKey::from_uri(snapshot.uri()), snapshot.clone());
        Ok(snapshot)
    }

    /// Applies a FULL document change and returns the new snapshot.
    ///
    /// # Panics
    ///
    /// Panics only if the retained source document cannot form its own exact
    /// whole-document UTF-8 span.
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
        let uri = params.text_document.uri;
        let key = LspUriKey::from_uri(&uri);
        let previous = self
            .documents
            .get(&key)
            .ok_or_else(|| DocumentError::DocumentNotOpen {
                uri: uri.to_string(),
            })?;
        let supplied_version = params.text_document.version;
        if supplied_version <= previous.version() {
            return Err(DocumentError::StaleVersion {
                current: previous.version(),
                supplied: supplied_version,
            });
        }
        let previous = previous.parsed_source().clone();
        let whole_document = previous
            .document()
            .span(SourceRange::new(0, previous.source().len()))
            .expect("a whole-document edit spans exact retained UTF-8 bytes");
        let edit = SourceEdit::new(whole_document, change.text);
        let parsed = self
            .syntax
            .reparse(&previous, &[edit], ParseOptions::default())?;
        let snapshot = DocumentSnapshot::new(uri, supplied_version, parsed, encoding);
        self.documents.insert(key, snapshot.clone());
        Ok(snapshot)
    }

    /// Mutably borrows this store's sole syntax session for project/profile loading.
    pub(crate) fn syntax_database_mut(&mut self) -> &mut SyntaxDatabase {
        &mut self.syntax
    }

    /// Validates every exact compiler parse before accepted publication mutates state.
    pub(crate) fn validate_parsed_source_adoptions(
        &self,
        adoptions: Vec<ParsedSourceAdoption>,
    ) -> Result<ValidatedParsedSourceAdoptions, ParsedSourceAdoptionError> {
        let mut seen = std::collections::BTreeSet::new();
        for adoption in &adoptions {
            let uri = adoption.uri.to_uri().to_string();
            if !seen.insert(adoption.uri.clone()) {
                return Err(ParsedSourceAdoptionError::DuplicateUri { uri });
            }
            let Some(current) = self.documents.get(&adoption.uri) else {
                return Err(ParsedSourceAdoptionError::DocumentNotOpen { uri });
            };
            if current.version() != adoption.version {
                return Err(ParsedSourceAdoptionError::VersionChanged {
                    uri,
                    expected: adoption.version,
                    actual: current.version(),
                });
            }
            if current.text() != adoption.parsed.source() {
                return Err(ParsedSourceAdoptionError::SourceChanged { uri });
            }
            if adoption.parsed.snapshot_id().lineage().database() != self.syntax.database_id() {
                return Err(ParsedSourceAdoptionError::ForeignSyntaxSession { uri });
            }
            if !self
                .syntax
                .current(adoption.parsed.snapshot_id().lineage())
                .is_ok_and(|parsed| parsed.is_same_snapshot(&adoption.parsed))
            {
                return Err(ParsedSourceAdoptionError::StaleSyntaxSnapshot { uri });
            }
        }
        Ok(ValidatedParsedSourceAdoptions(adoptions))
    }

    /// Atomically replaces provisional protocol parses with the accepted project leases.
    pub(crate) fn commit_parsed_source_adoptions(
        &mut self,
        adoptions: ValidatedParsedSourceAdoptions,
    ) {
        for adoption in adoptions.0 {
            let current = self
                .documents
                .get_mut(&adoption.uri)
                .expect("validated syntax adoption retains its live document");
            debug_assert_eq!(current.version(), adoption.version);
            debug_assert_eq!(current.text(), adoption.parsed.source());
            current.parsed = adoption.parsed;
        }
    }

    #[cfg(test)]
    fn syntax_database(&self) -> &SyntaxDatabase {
        &self.syntax
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
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, TextDocumentContentChangeEvent,
        TextDocumentItem, VersionedTextDocumentIdentifier,
    };

    fn open(store: &mut DocumentStore, uri: Uri, version: i32, text: &str) -> DocumentSnapshot {
        store
            .open(
                DidOpenTextDocumentParams {
                    text_document: TextDocumentItem::new(
                        uri,
                        "arcweft".to_owned(),
                        version,
                        text.to_owned(),
                    ),
                },
                PositionEncoding::Utf16,
            )
            .expect("initial document parse")
    }

    fn full_change(uri: Uri, version: i32, text: &str) -> DidChangeTextDocumentParams {
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.to_owned(),
            }],
        }
    }

    fn parse_attached_source(
        syntax: &mut SyntaxDatabase,
        logical_id: &str,
        text: &str,
    ) -> ParsedSource {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(logical_id).expect("logical source ID"),
                SourceName::path(logical_id),
                Arc::<str>::from(text),
            )
            .expect("attached source document"),
        );
        syntax
            .parse_initial(
                SourceSnapshotId::initial(document.display_name().clone()),
                document,
                ParseOptions::default(),
            )
            .expect("attached source parse")
    }

    #[test]
    fn open_retains_the_exact_initial_parse_in_the_store_session() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();
        let snapshot = open(&mut store, uri.clone(), 1, "flow opening {}\n");

        assert_eq!(
            snapshot.parsed_source().snapshot_id().lineage().database(),
            store.syntax_database().database_id()
        );
        assert!(Arc::ptr_eq(
            snapshot.source_document(),
            snapshot.parsed_source().document_lease()
        ));
        assert_eq!(
            snapshot.line_index().source(),
            snapshot.parsed_source().source()
        );
        let retained = store.get(&uri).expect("opened document is retained");
        assert!(
            retained
                .parsed_source()
                .is_same_snapshot(snapshot.parsed_source())
        );
    }

    #[test]
    fn accepted_project_parse_is_adopted_as_the_exact_live_lease() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();
        let provisional = open(&mut store, uri.clone(), 7, "flow opening {}\n");
        let accepted = parse_attached_source(
            store.syntax_database_mut(),
            "arcweft-project://story/src/story.arcw",
            provisional.text(),
        );
        assert!(!provisional.parsed_source().is_same_snapshot(&accepted));

        let validated = store
            .validate_parsed_source_adoptions(vec![ParsedSourceAdoption::new(
                LspUriKey::from_uri(&uri),
                provisional.version(),
                accepted.clone(),
            )])
            .expect("same-session current compiler parse is admissible");
        store.commit_parsed_source_adoptions(validated);

        let live = store.get(&uri).expect("open document remains live");
        assert_eq!(live.version(), provisional.version());
        assert!(live.parsed_source().is_same_snapshot(&accepted));
        assert!(Arc::ptr_eq(
            live.parsed_source().document_lease(),
            accepted.document_lease()
        ));
    }

    #[test]
    fn foreign_project_parse_rejection_preserves_the_live_lease() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();
        let live = open(&mut store, uri.clone(), 3, "flow opening {}\n");
        let mut foreign_syntax = SyntaxDatabase::try_new().expect("foreign syntax database");
        let foreign = parse_attached_source(
            &mut foreign_syntax,
            "arcweft-project://story/src/story.arcw",
            live.text(),
        );

        let error = store
            .validate_parsed_source_adoptions(vec![ParsedSourceAdoption::new(
                LspUriKey::from_uri(&uri),
                live.version(),
                foreign,
            )])
            .expect_err("foreign compiler parse must not enter the live store");
        assert!(matches!(
            error,
            ParsedSourceAdoptionError::ForeignSyntaxSession { .. }
        ));
        let retained = store.get(&uri).expect("live lease remains published");
        assert!(
            retained
                .parsed_source()
                .is_same_snapshot(live.parsed_source())
        );
    }

    #[test]
    fn full_sync_reparses_the_existing_lineage_and_publishes_one_exact_lease() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();
        let initial = open(&mut store, uri.clone(), 1, "flow opening {}\n");

        let snapshot = store
            .change(
                full_change(uri.clone(), 2, "flow @flow.opening opening {}\n"),
                PositionEncoding::Utf16,
            )
            .expect("full sync change");

        assert_eq!(snapshot.version(), 2);
        assert_eq!(
            snapshot
                .parsed_source()
                .source_snapshot_id()
                .generation()
                .get(),
            2
        );
        assert_eq!(
            snapshot.parsed_source().snapshot_id().lineage(),
            initial.parsed_source().snapshot_id().lineage()
        );
        assert!(
            !snapshot
                .parsed_source()
                .is_same_snapshot(initial.parsed_source())
        );
        let retained = store.get(&uri).expect("changed document is retained");
        assert!(Arc::ptr_eq(
            snapshot.source_document(),
            retained.source_document()
        ));
        assert!(
            retained
                .parsed_source()
                .is_same_snapshot(snapshot.parsed_source())
        );
        assert_eq!(
            snapshot.line_index().source(),
            snapshot.parsed_source().source()
        );
    }

    #[test]
    fn identical_full_sync_advances_only_the_lsp_version() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();
        let initial = open(&mut store, uri.clone(), 1, "flow opening {}\n");

        let changed = store
            .change(
                full_change(uri.clone(), 2, initial.text()),
                PositionEncoding::Utf16,
            )
            .expect("identical FULL sync");

        assert_eq!(changed.version(), 2);
        assert!(
            changed
                .parsed_source()
                .is_same_snapshot(initial.parsed_source())
        );
        assert!(Arc::ptr_eq(
            changed.source_document(),
            initial.source_document()
        ));
        assert_eq!(
            store.get(&uri).expect("new LSP version").version(),
            changed.version()
        );
    }

    #[test]
    fn rejected_second_open_does_not_replace_the_existing_lineage() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();
        let initial = open(&mut store, uri.clone(), 1, "flow opening {}\n");
        let lineage = initial.parsed_source().snapshot_id().lineage();

        let error = store
            .open(
                DidOpenTextDocumentParams {
                    text_document: TextDocumentItem::new(
                        uri.clone(),
                        "arcweft".to_owned(),
                        2,
                        "flow replacement {}\n".to_owned(),
                    ),
                },
                PositionEncoding::Utf16,
            )
            .expect_err("a second open cannot replace one live lineage");
        assert!(matches!(
            error,
            DocumentError::Syntax(ParseFailure::SourceMismatch)
        ));
        assert!(
            store
                .get(&uri)
                .expect("initial LSP snapshot")
                .parsed_source()
                .is_same_snapshot(initial.parsed_source())
        );
        assert!(
            store
                .syntax_database()
                .current(lineage)
                .expect("initial syntax snapshot")
                .is_same_snapshot(initial.parsed_source())
        );
    }

    #[test]
    fn rejected_sync_shape_and_stale_version_publish_neither_owner() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();
        let initial = open(&mut store, uri.clone(), 4, "flow opening {}\n");
        let lineage = initial.parsed_source().snapshot_id().lineage();

        let malformed = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 5,
            },
            content_changes: Vec::new(),
        };
        assert!(matches!(
            store.change(malformed, PositionEncoding::Utf16),
            Err(DocumentError::ExpectedFullSyncChange)
        ));
        assert!(matches!(
            store.change(
                full_change(uri.clone(), 4, "flow stale {}\n"),
                PositionEncoding::Utf16,
            ),
            Err(DocumentError::StaleVersion {
                current: 4,
                supplied: 4,
            })
        ));

        let retained = store.get(&uri).expect("initial snapshot remains published");
        assert!(
            retained
                .parsed_source()
                .is_same_snapshot(initial.parsed_source())
        );
        let current = store
            .syntax_database()
            .current(lineage)
            .expect("syntax lineage remains current");
        assert!(current.is_same_snapshot(initial.parsed_source()));
    }

    #[test]
    fn stale_syntax_lease_rejection_does_not_replace_store_or_database_state() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();
        let initial = open(&mut store, uri.clone(), 1, "flow opening {}\n");
        let old_lease = initial.parsed_source().clone();
        let span = old_lease
            .document()
            .span(SourceRange::new(0, old_lease.source().len()))
            .expect("whole source span");
        let external_current = store
            .syntax_database_mut()
            .reparse(
                &old_lease,
                &[SourceEdit::new(span, "flow externally_advanced {}\n")],
                ParseOptions::default(),
            )
            .expect("profile-side transaction advances the shared lineage");

        let error = store
            .change(
                full_change(uri.clone(), 2, "flow editor_change {}\n"),
                PositionEncoding::Utf16,
            )
            .expect_err("the store snapshot is stale against its sole syntax session");
        assert!(matches!(
            error,
            DocumentError::Syntax(ParseFailure::StaleSnapshot { .. })
        ));
        let retained = store
            .get(&uri)
            .expect("failed edit retains old LSP snapshot");
        assert!(
            retained
                .parsed_source()
                .is_same_snapshot(initial.parsed_source())
        );
        let current = store
            .syntax_database()
            .current(external_current.snapshot_id().lineage())
            .expect("failed edit retains the exact database current snapshot");
        assert!(current.is_same_snapshot(&external_current));
    }
}
