//! Incremental parse database, snapshot transactions, and syntax identities.

use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_source::identity::{SourceGeneration, SourceSnapshotId};
use arcweft_source::{SourceDocument, SourceEdit, SourceName, SourceSpan};
use thiserror::Error;

use super::bound::{ParsedSourceData, SyntaxDiagnostic};
#[cfg(test)]
use crate::attachment::SyntaxSnapshotData;
use crate::attachment::{
    AstKind, AstNode, AttachedExpressionNode, AttachedPatternNode, AttachedTypeRefNode,
    AttachmentFailure, StatementNode, SyntaxAccessError, SyntaxDatabaseId, SyntaxLineageId,
    SyntaxLookupError, SyntaxNode, SyntaxNodeHandle, SyntaxNodeId, SyntaxSnapshotId,
    TypedSyntaxTree,
};
use crate::parser::fragment::{ParseCompletion, ParseOptions};
use crate::parser::unbound_fragment::{AttachedFragment, FragmentKind, UnboundFragment};

use super::limits::SyntaxLimit;
use super::transaction;

/// Immutable parse result owned by one incremental syntax database lineage.
#[derive(Clone, Debug)]
pub struct ParsedSource(Arc<ParsedSourceData>);

impl ParsedSource {
    /// Qualified syntax snapshot committed by this parse transaction.
    pub fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.0.snapshot_id()
    }

    /// Source name and generation accepted by this syntax snapshot.
    pub fn source_snapshot_id(&self) -> &SourceSnapshotId {
        self.snapshot_id().source()
    }

    /// Exact content provenance retained independently from session lineage.
    pub fn document(&self) -> &SourceDocument {
        self.0.document()
    }

    /// Shared lease for the exact immutable document accepted by this parse.
    pub fn document_lease(&self) -> &Arc<SourceDocument> {
        self.0.document()
    }

    /// Exact UTF-8 source bytes for this immutable snapshot.
    pub fn source(&self) -> &str {
        self.document().text()
    }

    /// Whether two values refer to the exact same immutable grammar snapshot.
    pub fn is_same_snapshot(&self, other: &Self) -> bool {
        self.snapshot_id() == other.snapshot_id()
    }

    /// Attached typed source-file root for this exact immutable snapshot.
    ///
    /// # Panics
    ///
    /// Panics only if crate-internal construction publishes a snapshot without
    /// the source-file root validated before transaction commit.
    pub fn tree(&self) -> TypedSyntaxTree {
        self.0
            .syntax()
            .typed_tree()
            .expect("committed syntax snapshots retain a typed source-file root")
    }

    /// Attached raw source-file root for this exact immutable snapshot.
    pub fn root_syntax(&self) -> SyntaxNodeHandle {
        self.0.syntax().root_handle()
    }

    /// Resolves one stable node identity in this snapshot.
    pub fn syntax_node(&self, id: SyntaxNodeId) -> Result<SyntaxNodeHandle, SyntaxLookupError> {
        self.0.syntax().syntax_node(id)
    }

    /// Resolves one stable identity as its exact attached semantic type.
    pub fn attached_type_ref(
        &self,
        id: SyntaxNodeId,
    ) -> Result<AttachedTypeRefNode, SyntaxAccessError> {
        AttachedTypeRefNode::from_syntax(self.syntax_node(id)?)
    }

    /// Resolves one stable identity as its exact attached semantic Pattern.
    pub fn attached_pattern(
        &self,
        id: SyntaxNodeId,
    ) -> Result<AttachedPatternNode, SyntaxAccessError> {
        AttachedPatternNode::from_syntax(self.syntax_node(id)?)
    }

    /// Resolves one stable identity as its exact attached semantic expression.
    pub fn attached_expression(
        &self,
        id: SyntaxNodeId,
    ) -> Result<AttachedExpressionNode, SyntaxAccessError> {
        AttachedExpressionNode::from_syntax(self.syntax_node(id)?)
    }

    /// Resolves one stable identity as an exact attached statement family.
    pub fn statement_node(&self, id: SyntaxNodeId) -> Result<StatementNode, SyntaxAccessError> {
        StatementNode::new(self.syntax_node(id)?)
    }

    /// Resolves one stable identity as a syntax-owned typed node.
    pub fn typed_node<K: AstKind>(
        &self,
        id: SyntaxNodeId,
    ) -> Result<AstNode<K>, SyntaxLookupError> {
        self.0.syntax().typed_node(id)
    }

    /// Binds a Rowan node only when it belongs to this exact root allocation.
    pub fn bind_rowan(&self, node: &SyntaxNode) -> Result<SyntaxNodeHandle, SyntaxLookupError> {
        self.0.syntax().bind_rowan(node)
    }

    /// Resolves a typed handle only when it belongs to this exact snapshot.
    pub fn resolve_exact<K: AstKind>(
        &self,
        node: &AstNode<K>,
    ) -> Result<AstNode<K>, SyntaxLookupError> {
        let exact = self.0.syntax().resolve_exact(&node.syntax())?;
        self.0.syntax().typed_node(exact.id())
    }

    /// Resolves a raw handle only when it belongs to this exact snapshot.
    pub fn resolve_exact_syntax(
        &self,
        node: &SyntaxNodeHandle,
    ) -> Result<SyntaxNodeHandle, SyntaxLookupError> {
        self.0.syntax().resolve_exact(node)
    }

    /// Recoverable diagnostics emitted by this exact parse transaction.
    pub fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        self.0.diagnostics()
    }

    /// Whether this snapshot is clean or contains recovered syntax.
    pub fn status(&self) -> ParseStatus {
        self.0.status()
    }

    #[cfg(test)]
    pub(crate) fn attached(&self) -> &Arc<SyntaxSnapshotData> {
        self.0.syntax()
    }

    #[cfg(test)]
    pub(super) const fn data(&self) -> &Arc<ParsedSourceData> {
        &self.0
    }
}

/// Owner of source generations and never-reused CST identities for one session.
#[derive(Debug)]
pub struct SyntaxDatabase {
    lineages: BTreeMap<SourceName, SourceLineage>,
    limits: SyntaxTransactionLimits,
    transaction: transaction::SyntaxTransactionState,
}

#[derive(Debug)]
struct SourceLineage {
    current: ParsedSource,
    transaction: transaction::SyntaxLineageState,
}

#[derive(Clone, Copy, Debug)]
struct SyntaxTransactionLimits {
    source_generation: u32,
}

impl Default for SyntaxTransactionLimits {
    fn default() -> Self {
        Self {
            source_generation: u32::MAX,
        }
    }
}

/// Whether a parsed snapshot is executable or contains recovered error nodes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseStatus {
    Clean,
    Recovered,
}

/// Structural reason an incremental edit transaction cannot be applied.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InvalidEditSet {
    #[error("edit {index} is not sorted after the preceding edit")]
    Unsorted { index: usize },
    #[error("edit {index} overlaps the preceding edit")]
    Overlapping { index: usize },
    #[error("edit {index} range {start}..{end} is outside source length {source_len}")]
    OutOfBounds {
        index: usize,
        start: usize,
        end: usize,
        source_len: usize,
    },
    #[error("edit {index} offset {offset} is not a UTF-8 boundary")]
    NonUtf8Boundary { index: usize, offset: usize },
}

/// Fatal parse failures that commit no generation or syntax identities.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ParseFailure {
    #[error("the source does not match the parse database lineage")]
    SourceMismatch,
    #[error("the supplied syntax snapshot is stale")]
    StaleSnapshot {
        current: SyntaxSnapshotId,
        supplied: SyntaxSnapshotId,
    },
    #[error(transparent)]
    InvalidEdits(#[from] InvalidEditSet),
    #[error("syntax limit {0:?} was exceeded")]
    LimitExceeded(SyntaxLimit),
    #[error("source generation allocation is exhausted")]
    SourceGenerationExhausted,
    #[error("syntax database identity allocation is exhausted")]
    DatabaseIdentityExhausted,
    #[error("syntax lineage identity allocation is exhausted")]
    LineageIdentityExhausted,
    #[error("syntax node identity allocation is exhausted")]
    NodeIdentityExhausted,
    #[error("the grammar event stream does not reproduce the exact source bytes")]
    LosslessnessViolation,
    #[error(transparent)]
    Attachment(#[from] AttachmentFailure),
    #[error(transparent)]
    Invariant(#[from] SyntaxInvariantFailure),
}

/// Fatal syntax transaction invariant selected by the owning transaction.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SyntaxInvariantFailure {
    #[error("the syntax identity allocator regressed")]
    AllocatorRegression,
    #[error("the grammar identity map is inconsistent")]
    IdentityMapMismatch,
    #[error("the parsed snapshot does not retain its exact source ownership")]
    SnapshotOwnershipMismatch,
}

/// Private predecessor of the final public fragment-attachment error.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum FragmentAttachmentFailure {
    #[error("the fragment source identity or span does not match the target document")]
    SourceMismatch,
    #[error("only a complete standalone fragment can be attached: {completion:?}")]
    FragmentNotComplete { completion: ParseCompletion },
    #[error("the target source bytes do not exactly match the standalone fragment")]
    FragmentTextMismatch,
    #[error(transparent)]
    Transaction(#[from] ParseFailure),
}

/// Failure to allocate a fresh syntax database session identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SyntaxDatabaseCreateError {
    #[error("syntax database identity allocation is exhausted")]
    IdentityExhausted,
}

impl SyntaxDatabase {
    /// Allocates a fresh syntax database with a never-reused session identity.
    pub fn try_new() -> Result<Self, SyntaxDatabaseCreateError> {
        Ok(Self {
            lineages: BTreeMap::new(),
            limits: SyntaxTransactionLimits::default(),
            transaction: transaction::SyntaxTransactionState::try_new()
                .ok_or(SyntaxDatabaseCreateError::IdentityExhausted)?,
        })
    }

    /// Process-local identity that qualifies every lineage owned by this database.
    pub fn database_id(&self) -> SyntaxDatabaseId {
        self.transaction.database_id()
    }

    /// Returns the exact current whole-source snapshot for one database lineage.
    pub fn current(&self, lineage: SyntaxLineageId) -> Result<ParsedSource, SyntaxLookupError> {
        if lineage.database() != self.database_id() {
            return Err(SyntaxLookupError::WrongDatabase {
                expected: self.database_id(),
                actual: lineage.database(),
            });
        }
        self.lineages
            .values()
            .find(|candidate| candidate.current.snapshot_id().lineage() == lineage)
            .map(|candidate| candidate.current.clone())
            .ok_or(SyntaxLookupError::UnknownLineage { lineage })
    }

    /// Resolves a typed node only against the current generation of its lineage.
    pub fn resolve_current<K: AstKind>(
        &self,
        node: &AstNode<K>,
    ) -> Result<AstNode<K>, SyntaxLookupError> {
        let supplied = node.snapshot_id();
        let current = self.current(supplied.lineage())?;
        let current_generation = current.source_snapshot_id().generation();
        let supplied_generation = supplied.source().generation();
        if current_generation != supplied_generation {
            return Err(SyntaxLookupError::StaleGeneration {
                current: current_generation,
                supplied: supplied_generation,
            });
        }
        current.typed_node(node.id())
    }

    #[cfg(test)]
    fn with_test_limits(limits: SyntaxTransactionLimits) -> Self {
        let mut database = Self::try_new().expect("test syntax database identity");
        database.limits = limits;
        database
    }

    /// Parses the first generation of one source lineage atomically.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the public transaction contract takes ownership of the snapshot and immutable source"
    )]
    pub fn parse_initial(
        &mut self,
        snapshot: SourceSnapshotId,
        document: Arc<SourceDocument>,
        options: ParseOptions,
    ) -> Result<ParsedSource, ParseFailure> {
        self.parse_initial_with_transaction_fault(
            &snapshot,
            &document,
            options,
            transaction::TransactionFault::None,
        )
    }

    /// Attaches one complete standalone fragment to a fresh private lineage.
    ///
    /// Attachment projects the retained grammar events into the exact target
    /// span. It never parses the target document bytes again and cannot produce
    /// a whole-source [`ParsedSource`].
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the private fragment entrypoint precedes the atomic parser/tooling switch"
        )
    )]
    pub(crate) fn attach_fragment<K: FragmentKind>(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
        span: &SourceSpan,
        fragment: UnboundFragment<K>,
    ) -> Result<AttachedFragment<K>, FragmentAttachmentFailure> {
        self.attach_fragment_with_transaction_fault(
            snapshot,
            document,
            span,
            fragment,
            transaction::TransactionFault::None,
        )
    }

    fn attach_fragment_with_transaction_fault<K: FragmentKind>(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
        span: &SourceSpan,
        fragment: UnboundFragment<K>,
        transaction_fault: transaction::TransactionFault,
    ) -> Result<AttachedFragment<K>, FragmentAttachmentFailure> {
        if snapshot.name() != document.display_name() || span.validate_for(document).is_err() {
            return Err(FragmentAttachmentFailure::SourceMismatch);
        }
        let (text, tree, completion) = fragment.into_parts();
        if completion != ParseCompletion::Complete {
            return Err(FragmentAttachmentFailure::FragmentNotComplete { completion });
        }
        if &document.text()[span.range().as_range()] != text.as_ref() {
            return Err(FragmentAttachmentFailure::FragmentTextMismatch);
        }
        let staged = self.transaction.stage_fragment::<K>(
            snapshot,
            document,
            span,
            &tree,
            transaction_fault,
        )?;
        Ok(self.transaction.commit_fragment(staged))
    }

    fn parse_initial_with_transaction_fault(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
        options: ParseOptions,
        transaction_fault: transaction::TransactionFault,
    ) -> Result<ParsedSource, ParseFailure> {
        if snapshot.generation() != SourceGeneration::INITIAL
            || snapshot.name() != document.display_name()
            || self.lineages.contains_key(snapshot.name())
        {
            return Err(ParseFailure::SourceMismatch);
        }
        let staged =
            self.transaction
                .stage_initial(snapshot, document, options, transaction_fault)?;
        let result = ParsedSource(Arc::clone(staged.current()));
        let transaction = self.transaction.commit_initial(staged);
        self.lineages.insert(
            snapshot.name().clone(),
            SourceLineage {
                current: result.clone(),
                transaction,
            },
        );
        Ok(result)
    }

    /// Applies simultaneous checked edits and reconciles stable CST identities.
    pub fn reparse(
        &mut self,
        previous: &ParsedSource,
        edits: &[SourceEdit],
        options: ParseOptions,
    ) -> Result<ParsedSource, ParseFailure> {
        self.reparse_with_transaction_fault(
            previous,
            edits,
            options,
            transaction::TransactionFault::None,
        )
    }

    fn reparse_with_transaction_fault(
        &mut self,
        previous: &ParsedSource,
        edits: &[SourceEdit],
        options: ParseOptions,
        transaction_fault: transaction::TransactionFault,
    ) -> Result<ParsedSource, ParseFailure> {
        let lineage = self
            .lineages
            .get(previous.source_snapshot_id().name())
            .ok_or(ParseFailure::SourceMismatch)?;
        if lineage.current.snapshot_id().lineage() != previous.snapshot_id().lineage() {
            return Err(ParseFailure::SourceMismatch);
        }
        if !lineage.current.is_same_snapshot(previous)
            || lineage.current.source() != previous.source()
            || !Arc::ptr_eq(lineage.transaction.current(), previous.data_internal())
        {
            return Err(ParseFailure::StaleSnapshot {
                current: lineage.current.snapshot_id().clone(),
                supplied: previous.snapshot_id().clone(),
            });
        }
        validate_edits(previous, edits)?;
        if edits.is_empty() {
            return Ok(lineage.current.clone());
        }
        let next_text = apply_edits(previous.source(), edits);
        if next_text == previous.source() {
            return Ok(lineage.current.clone());
        }

        if previous.source_snapshot_id().generation().get() >= self.limits.source_generation {
            return Err(ParseFailure::SourceGenerationExhausted);
        }
        let document = Arc::new(
            SourceDocument::try_new(
                previous.document().identity().id().clone(),
                previous.document().display_name().clone(),
                Arc::<str>::from(next_text),
            )
            .map_err(|_| SyntaxInvariantFailure::SnapshotOwnershipMismatch)?,
        );
        let snapshot = previous
            .source_snapshot_id()
            .checked_next()
            .map_err(|_| ParseFailure::SourceGenerationExhausted)?;
        let staged =
            lineage
                .transaction
                .stage_reparse(&snapshot, &document, options, transaction_fault)?;
        let result = ParsedSource(Arc::clone(staged.current()));
        let lineage = self
            .lineages
            .get_mut(previous.source_snapshot_id().name())
            .ok_or(SyntaxInvariantFailure::SnapshotOwnershipMismatch)?;
        lineage.current = result.clone();
        lineage.transaction = staged.into_lineage();
        Ok(result)
    }

    #[cfg(test)]
    fn parse_initial_with_attachment_failure(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
    ) -> Result<ParsedSource, ParseFailure> {
        self.parse_initial_with_transaction_fault(
            snapshot,
            document,
            ParseOptions::default(),
            transaction::TransactionFault::MissingAttachment,
        )
    }

    #[cfg(test)]
    fn reparse_with_attachment_failure(
        &mut self,
        previous: &ParsedSource,
        edits: &[SourceEdit],
    ) -> Result<ParsedSource, ParseFailure> {
        self.reparse_with_transaction_fault(
            previous,
            edits,
            ParseOptions::default(),
            transaction::TransactionFault::MissingAttachment,
        )
    }

    #[cfg(test)]
    fn attach_fragment_with_attachment_failure<K: FragmentKind>(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
        span: &SourceSpan,
        fragment: UnboundFragment<K>,
    ) -> Result<AttachedFragment<K>, FragmentAttachmentFailure> {
        self.attach_fragment_with_transaction_fault(
            snapshot,
            document,
            span,
            fragment,
            transaction::TransactionFault::MissingAttachment,
        )
    }
}

impl ParsedSource {
    const fn data_internal(&self) -> &Arc<ParsedSourceData> {
        &self.0
    }
}

fn validate_edits(previous: &ParsedSource, edits: &[SourceEdit]) -> Result<(), ParseFailure> {
    let source = previous.source();
    let mut prior: Option<(usize, usize)> = None;
    for (index, edit) in edits.iter().enumerate() {
        if edit.span().source() != previous.document().identity() {
            return Err(ParseFailure::SourceMismatch);
        }
        let range = edit.span().range();
        let start = range.start();
        let end = range.end();
        if start > end || end > source.len() {
            return Err(InvalidEditSet::OutOfBounds {
                index,
                start,
                end,
                source_len: source.len(),
            }
            .into());
        }
        if let Some((prior_start, prior_end)) = prior {
            if (start, end) < (prior_start, prior_end) {
                return Err(InvalidEditSet::Unsorted { index }.into());
            }
            if start < prior_end {
                return Err(InvalidEditSet::Overlapping { index }.into());
            }
        }
        for offset in [start, end] {
            if !source.is_char_boundary(offset) {
                return Err(InvalidEditSet::NonUtf8Boundary { index, offset }.into());
            }
        }
        prior = Some((start, end));
    }
    Ok(())
}

fn apply_edits(source: &str, edits: &[SourceEdit]) -> String {
    let replacement_bytes = edits
        .iter()
        .map(|edit| edit.replacement().len())
        .sum::<usize>();
    let removed_bytes = edits
        .iter()
        .map(|edit| {
            let range = edit.span().range();
            range.end() - range.start()
        })
        .sum::<usize>();
    let mut output = String::with_capacity(
        source
            .len()
            .saturating_sub(removed_bytes)
            .saturating_add(replacement_bytes),
    );
    let mut cursor = 0;
    for edit in edits {
        let range = edit.span().range();
        output.push_str(&source[cursor..range.start()]);
        output.push_str(edit.replacement());
        cursor = range.end();
    }
    output.push_str(&source[cursor..]);
    output
}

#[cfg(test)]
#[path = "database_tests.rs"]
mod tests;
