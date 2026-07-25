//! Incremental parse database, snapshot transactions, and syntax identities.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use arcweft_source::identity::{SourceGeneration, SourceSnapshotId};
use arcweft_source::{SourceDocument, SourceEdit, SourceName, SourceSpan};
use thiserror::Error;

use super::bound::{
    BoundExpressionFragment, BoundFragment, BoundFragmentKind, BoundParsedSource,
    BoundPatternFragment, BoundStatementFragment, BoundTypeFragment, ExpressionFragment,
    PatternFragment, StatementFragment, TypeFragment,
};
#[cfg(test)]
use crate::attachment::SyntaxSnapshotData;

use super::limits::SyntaxLimit;
use super::transaction;

/// Immutable parse result owned by one incremental syntax database lineage.
#[derive(Clone, Debug)]
pub struct ParsedSource {
    bound: Rc<BoundParsedSource>,
}

impl ParsedSource {
    /// Source name and generation accepted by the parse transaction.
    pub fn snapshot(&self) -> &SourceSnapshotId {
        self.bound.snapshot_id().source()
    }

    /// Exact content provenance retained independently from session lineage.
    pub fn document(&self) -> &SourceDocument {
        self.bound.document()
    }

    /// Exact UTF-8 source bytes for this immutable snapshot.
    pub fn source(&self) -> &str {
        self.document().text()
    }

    /// Whether this snapshot is clean or contains recovered syntax.
    pub fn status(&self) -> ParseStatus {
        self.bound.status()
    }

    #[cfg(test)]
    pub(crate) fn attached(&self) -> &Arc<SyntaxSnapshotData> {
        self.bound.syntax()
    }

    #[cfg(test)]
    pub(super) const fn bound(&self) -> &Rc<BoundParsedSource> {
        &self.bound
    }
}

/// Owner of source generations and never-reused CST identities for one session.
#[derive(Debug, Default)]
pub struct SyntaxDatabase {
    lineages: BTreeMap<SourceName, SourceLineage>,
    limits: SyntaxTransactionLimits,
    shadow: transaction::ShadowDatabaseState,
}

#[derive(Debug)]
struct SourceLineage {
    current: Arc<ParsedSource>,
    shadow: transaction::ShadowLineageState,
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

/// Identity allocation families reported by fatal parse failures.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxIdentityKind {
    Node,
    SourceGeneration,
}

impl SyntaxIdentityKind {
    /// Stable diagnostic label for this identity family.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Node => "syntax node",
            Self::SourceGeneration => "source generation",
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
    #[error(transparent)]
    InvalidEdits(#[from] InvalidEditSet),
    #[error("the source does not match the parse database lineage")]
    SourceMismatch,
    #[error("syntax limit {0:?} was exceeded")]
    LimitExceeded(SyntaxLimit),
    #[error("{} identity allocation is exhausted", .0.as_str())]
    IdentityExhausted(SyntaxIdentityKind),
    #[error("the syntax identity transaction violated an internal invariant")]
    InternalInvariant,
}

impl SyntaxDatabase {
    #[cfg(test)]
    fn with_test_limits(limits: SyntaxTransactionLimits) -> Self {
        Self {
            lineages: BTreeMap::new(),
            limits,
            shadow: transaction::ShadowDatabaseState::default(),
        }
    }

    /// Parses the first generation of one source lineage atomically.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the public transaction contract takes ownership of the snapshot and immutable source"
    )]
    pub fn parse_initial(
        &mut self,
        snapshot: SourceSnapshotId,
        document: SourceDocument,
    ) -> Result<Arc<ParsedSource>, ParseFailure> {
        self.parse_initial_with_shadow_fault(&snapshot, &document, transaction::ShadowFault::None)
    }

    /// Attaches one standalone expression to a database-owned private lineage.
    ///
    /// The caller must provide the source identity explicitly. This private
    /// predecessor never fabricates a source lineage and cannot be passed to
    /// source-file HIR lowering as a [`ParsedSource`].
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the private fragment entrypoint precedes the atomic parser/tooling switch"
        )
    )]
    pub(crate) fn parse_bound_expression_fragment(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &SourceDocument,
        span: &SourceSpan,
    ) -> Result<BoundExpressionFragment, ParseFailure> {
        self.parse_bound_fragment_with_shadow_fault::<ExpressionFragment>(
            snapshot,
            document,
            span,
            transaction::ShadowFault::None,
        )
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the private fragment entrypoint precedes the atomic parser/tooling switch"
        )
    )]
    pub(crate) fn parse_bound_type_fragment(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &SourceDocument,
        span: &SourceSpan,
    ) -> Result<BoundTypeFragment, ParseFailure> {
        self.parse_bound_fragment_with_shadow_fault::<TypeFragment>(
            snapshot,
            document,
            span,
            transaction::ShadowFault::None,
        )
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the private fragment entrypoint precedes the atomic parser/tooling switch"
        )
    )]
    pub(crate) fn parse_bound_pattern_fragment(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &SourceDocument,
        span: &SourceSpan,
    ) -> Result<BoundPatternFragment, ParseFailure> {
        self.parse_bound_fragment_with_shadow_fault::<PatternFragment>(
            snapshot,
            document,
            span,
            transaction::ShadowFault::None,
        )
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the private fragment entrypoint precedes the atomic parser/tooling switch"
        )
    )]
    pub(crate) fn parse_bound_statement_fragment(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &SourceDocument,
        span: &SourceSpan,
    ) -> Result<BoundStatementFragment, ParseFailure> {
        self.parse_bound_fragment_with_shadow_fault::<StatementFragment>(
            snapshot,
            document,
            span,
            transaction::ShadowFault::None,
        )
    }

    fn parse_bound_fragment_with_shadow_fault<K: BoundFragmentKind>(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &SourceDocument,
        span: &SourceSpan,
        shadow_fault: transaction::ShadowFault,
    ) -> Result<BoundFragment<K>, ParseFailure> {
        if snapshot.name() != document.display_name() || span.validate_for(document).is_err() {
            return Err(ParseFailure::SourceMismatch);
        }
        let staged = self
            .shadow
            .stage_fragment::<K>(snapshot, document, span, shadow_fault)?;
        Ok(self.shadow.commit_fragment(staged))
    }

    #[expect(
        clippy::arc_with_non_send_sync,
        reason = "the contract requires immutable Arc snapshots while Rowan red nodes remain session-thread-affine"
    )]
    fn parse_initial_with_shadow_fault(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &SourceDocument,
        shadow_fault: transaction::ShadowFault,
    ) -> Result<Arc<ParsedSource>, ParseFailure> {
        if snapshot.generation() != SourceGeneration::INITIAL
            || snapshot.name() != document.display_name()
            || self.lineages.contains_key(snapshot.name())
        {
            return Err(ParseFailure::SourceMismatch);
        }
        let shadow = self
            .shadow
            .stage_initial(snapshot, document, shadow_fault)?;
        let result = Arc::new(ParsedSource {
            bound: Rc::clone(shadow.current()),
        });
        let shadow = self.shadow.commit_initial(shadow);
        self.lineages.insert(
            snapshot.name().clone(),
            SourceLineage {
                current: Arc::clone(&result),
                shadow,
            },
        );
        Ok(result)
    }

    /// Applies simultaneous checked edits and reconciles stable CST identities.
    pub fn reparse(
        &mut self,
        previous: &ParsedSource,
        edits: &[SourceEdit],
    ) -> Result<Arc<ParsedSource>, ParseFailure> {
        self.reparse_with_shadow_fault(previous, edits, transaction::ShadowFault::None)
    }

    #[expect(
        clippy::arc_with_non_send_sync,
        reason = "the contract requires immutable Arc snapshots while Rowan red nodes remain session-thread-affine"
    )]
    fn reparse_with_shadow_fault(
        &mut self,
        previous: &ParsedSource,
        edits: &[SourceEdit],
        shadow_fault: transaction::ShadowFault,
    ) -> Result<Arc<ParsedSource>, ParseFailure> {
        let lineage = self
            .lineages
            .get(previous.snapshot().name())
            .ok_or(ParseFailure::SourceMismatch)?;
        if lineage.current.snapshot() != previous.snapshot()
            || lineage.current.source() != previous.source()
            || !Rc::ptr_eq(lineage.shadow.current(), previous.bound_internal())
        {
            return Err(ParseFailure::SourceMismatch);
        }
        validate_edits(previous, edits)?;
        if edits.is_empty() {
            return Ok(Arc::clone(&lineage.current));
        }
        let next_text = apply_edits(previous.source(), edits);
        if next_text == previous.source() {
            return Ok(Arc::clone(&lineage.current));
        }

        if previous.snapshot().generation().get() >= self.limits.source_generation {
            return Err(ParseFailure::IdentityExhausted(
                SyntaxIdentityKind::SourceGeneration,
            ));
        }
        let document = SourceDocument::try_new(
            previous.document().identity().id().clone(),
            previous.document().display_name().clone(),
            Arc::<str>::from(next_text),
        )
        .map_err(|_| ParseFailure::InternalInvariant)?;
        let snapshot = previous
            .snapshot()
            .checked_next()
            .map_err(|_| ParseFailure::IdentityExhausted(SyntaxIdentityKind::SourceGeneration))?;
        let shadow = lineage
            .shadow
            .stage_reparse(&snapshot, &document, shadow_fault)?;
        let result = Arc::new(ParsedSource {
            bound: Rc::clone(shadow.current()),
        });
        let lineage = self
            .lineages
            .get_mut(previous.snapshot().name())
            .ok_or(ParseFailure::InternalInvariant)?;
        lineage.current = Arc::clone(&result);
        lineage.shadow = shadow.into_lineage();
        Ok(result)
    }

    #[cfg(test)]
    fn parse_initial_with_attachment_failure(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &SourceDocument,
    ) -> Result<Arc<ParsedSource>, ParseFailure> {
        self.parse_initial_with_shadow_fault(
            snapshot,
            document,
            transaction::ShadowFault::MissingAttachment,
        )
    }

    #[cfg(test)]
    fn reparse_with_attachment_failure(
        &mut self,
        previous: &ParsedSource,
        edits: &[SourceEdit],
    ) -> Result<Arc<ParsedSource>, ParseFailure> {
        self.reparse_with_shadow_fault(previous, edits, transaction::ShadowFault::MissingAttachment)
    }

    #[cfg(test)]
    fn parse_bound_fragment_with_attachment_failure<K: BoundFragmentKind>(
        &mut self,
        snapshot: &SourceSnapshotId,
        document: &SourceDocument,
        span: &SourceSpan,
    ) -> Result<BoundFragment<K>, ParseFailure> {
        self.parse_bound_fragment_with_shadow_fault::<K>(
            snapshot,
            document,
            span,
            transaction::ShadowFault::MissingAttachment,
        )
    }
}

impl ParsedSource {
    const fn bound_internal(&self) -> &Rc<BoundParsedSource> {
        &self.bound
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
