//! Incremental parse database, snapshot transactions, and syntax identities.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arcweft_source::identity::{SourceGeneration, SourceSnapshotId};
use arcweft_source::{SourceDocument, SourceEdit, SourceName};
use core::num::NonZeroU64;
use thiserror::Error;

use crate::ast::items::TypedSyntaxTree;
use crate::cst::SyntaxNode;
use crate::parser::recovery::{ParseError, ParseErrorKind};

use super::limits::SyntaxLimit;
use super::{reconcile, shape};

/// Stable node identity within one in-memory syntax database lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxNodeId(pub(super) NonZeroU64);

/// Node-to-ID inventory for one immutable parsed CST snapshot.
#[derive(Clone, Debug)]
pub struct SyntaxIdentityMap {
    nodes: HashMap<SyntaxNode, SyntaxNodeId>,
}

impl SyntaxIdentityMap {
    pub(super) fn new(nodes: HashMap<SyntaxNode, SyntaxNodeId>) -> Self {
        Self { nodes }
    }

    /// Returns the stable session identity assigned to this snapshot node.
    pub fn id_for(&self, node: &SyntaxNode) -> Option<SyntaxNodeId> {
        self.nodes.get(node).copied()
    }

    /// Number of identity-bearing CST nodes in this snapshot.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the snapshot has no identity-bearing nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Immutable parse result owned by one incremental syntax database lineage.
#[derive(Clone, Debug)]
pub struct ParsedSource {
    snapshot: SourceSnapshotId,
    document: SourceDocument,
    parsed: crate::source::ParsedSource,
    identities: Arc<SyntaxIdentityMap>,
    status: ParseStatus,
}

impl ParsedSource {
    /// Source name and generation accepted by the parse transaction.
    pub const fn snapshot(&self) -> &SourceSnapshotId {
        &self.snapshot
    }

    /// Exact content provenance retained independently from session lineage.
    pub const fn document(&self) -> &SourceDocument {
        &self.document
    }

    /// Exact UTF-8 source bytes for this immutable snapshot.
    pub fn source(&self) -> &str {
        self.document.text()
    }

    /// Lossless CST root.
    pub const fn root(&self) -> &SyntaxNode {
        self.parsed.syntax()
    }

    /// Typed surface tree projected from the lossless CST.
    pub const fn typed_tree(&self) -> &TypedSyntaxTree {
        self.parsed.typed_tree()
    }

    /// Stable node identities for this snapshot.
    pub fn identities(&self) -> &Arc<SyntaxIdentityMap> {
        &self.identities
    }

    /// Recoverable syntax diagnostics in deterministic parser order.
    pub fn diagnostics(&self) -> &[ParseError] {
        self.parsed.errors()
    }

    /// Whether this snapshot is clean or contains recovered syntax.
    pub const fn status(&self) -> ParseStatus {
        self.status
    }
}

/// Owner of source generations and never-reused CST identities for one session.
#[derive(Debug, Default)]
pub struct SyntaxDatabase {
    lineages: BTreeMap<SourceName, SourceLineage>,
    limits: SyntaxTransactionLimits,
}

#[derive(Debug)]
struct SourceLineage {
    current: Arc<ParsedSource>,
    allocator: NodeAllocator,
}

#[derive(Clone, Copy, Debug)]
struct SyntaxTransactionLimits {
    top_level_items: usize,
    diagnostics: usize,
    source_generation: u32,
}

impl Default for SyntaxTransactionLimits {
    fn default() -> Self {
        Self {
            top_level_items: SyntaxLimit::TopLevelItems.maximum(),
            diagnostics: SyntaxLimit::Diagnostics.maximum(),
            source_generation: u32::MAX,
        }
    }
}

#[derive(Clone, Debug)]
struct NodeAllocator {
    next: Option<NonZeroU64>,
}

impl Default for NodeAllocator {
    fn default() -> Self {
        Self {
            next: Some(NonZeroU64::MIN),
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
        }
    }

    /// Parses the first generation of one source lineage atomically.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the public transaction contract takes ownership of the snapshot and immutable source"
    )]
    #[expect(
        clippy::arc_with_non_send_sync,
        reason = "the contract requires immutable Arc snapshots while rowan red nodes remain session-thread-affine"
    )]
    pub fn parse_initial(
        &mut self,
        snapshot: SourceSnapshotId,
        document: SourceDocument,
    ) -> Result<Arc<ParsedSource>, ParseFailure> {
        if snapshot.generation() != SourceGeneration::INITIAL
            || snapshot.name() != document.display_name()
            || self.lineages.contains_key(snapshot.name())
        {
            return Err(ParseFailure::SourceMismatch);
        }
        let parsed = parse_checked(document.text(), self.limits)?;
        let shape = shape::ShapeNode::from_syntax(parsed.syntax().clone());
        let mut allocator = NodeAllocator::default();
        let identities = reconcile::allocate_initial(&shape, &mut || allocator.allocate())?;
        let result = Arc::new(ParsedSource {
            snapshot: snapshot.clone(),
            document,
            status: parse_status(&parsed),
            parsed,
            identities: Arc::new(identities),
        });
        self.lineages.insert(
            snapshot.name().clone(),
            SourceLineage {
                current: Arc::clone(&result),
                allocator,
            },
        );
        Ok(result)
    }

    /// Applies simultaneous checked edits and reconciles stable CST identities.
    #[expect(
        clippy::arc_with_non_send_sync,
        reason = "the contract requires immutable Arc snapshots while rowan red nodes remain session-thread-affine"
    )]
    pub fn reparse(
        &mut self,
        previous: &ParsedSource,
        edits: &[SourceEdit],
    ) -> Result<Arc<ParsedSource>, ParseFailure> {
        let lineage = self
            .lineages
            .get(previous.snapshot().name())
            .ok_or(ParseFailure::SourceMismatch)?;
        if lineage.current.snapshot() != previous.snapshot()
            || lineage.current.source() != previous.source()
            || !Arc::ptr_eq(lineage.current.identities(), previous.identities())
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
        let parsed = parse_checked(document.text(), self.limits)?;
        let snapshot = previous
            .snapshot()
            .checked_next()
            .map_err(|_| ParseFailure::IdentityExhausted(SyntaxIdentityKind::SourceGeneration))?;
        let old_shape = shape::ShapeNode::from_syntax(previous.root().clone());
        let new_shape = shape::ShapeNode::from_syntax(parsed.syntax().clone());
        let mut allocator = lineage.allocator.clone();
        let identities =
            reconcile::reconcile(&old_shape, &new_shape, previous.identities(), &mut || {
                allocator.allocate()
            })?;
        let result = Arc::new(ParsedSource {
            snapshot,
            document,
            status: parse_status(&parsed),
            parsed,
            identities: Arc::new(identities),
        });
        let lineage = self
            .lineages
            .get_mut(previous.snapshot().name())
            .ok_or(ParseFailure::InternalInvariant)?;
        lineage.current = Arc::clone(&result);
        lineage.allocator = allocator;
        Ok(result)
    }
}

impl NodeAllocator {
    fn allocate(&mut self) -> Result<SyntaxNodeId, ParseFailure> {
        let slot = self
            .next
            .ok_or(ParseFailure::IdentityExhausted(SyntaxIdentityKind::Node))?;
        self.next = slot.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(SyntaxNodeId(slot))
    }
}

fn parse_checked(
    source: &str,
    limits: SyntaxTransactionLimits,
) -> Result<crate::source::ParsedSource, ParseFailure> {
    let parsed = crate::parser::parse_source(source.to_owned());
    if parsed.errors().len() > limits.diagnostics {
        return Err(ParseFailure::LimitExceeded(SyntaxLimit::Diagnostics));
    }
    if parsed.typed_tree().items().len() > limits.top_level_items {
        return Err(ParseFailure::LimitExceeded(SyntaxLimit::TopLevelItems));
    }
    if parsed.syntax_stats().prefix_depth_limit_failures > 0 {
        return Err(ParseFailure::LimitExceeded(SyntaxLimit::PrefixDepth));
    }
    if parsed
        .errors()
        .iter()
        .any(|error| error.kind() == ParseErrorKind::AssertionTooManyConditions)
    {
        return Err(ParseFailure::LimitExceeded(
            SyntaxLimit::AssertionConditions,
        ));
    }
    Ok(parsed)
}

fn parse_status(parsed: &crate::source::ParsedSource) -> ParseStatus {
    if parsed.errors().is_empty() {
        ParseStatus::Clean
    } else {
        ParseStatus::Recovered
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
