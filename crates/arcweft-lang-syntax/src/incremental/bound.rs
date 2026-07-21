//! Crate-private bound parse product staged before the atomic public switch.
//!
//! This type is deliberately not exported from `incremental`. It lets the
//! grammar transaction retain one immutable document/snapshot/diagnostic
//! product without adapting the attached grammar back into the detached AST
//! that still feeds HIR. The public replacement must delete that detached
//! authority instead of wrapping it.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the bound product remains crate-private until the atomic syntax/HIR public switch"
    )
)]

use core::marker::PhantomData;
use std::sync::Arc;

use arcweft_source::{SourceDocument, SourceRange, SourceSpan, SourceSpanError};

use crate::attachment::{SyntaxNodeHandle, SyntaxSnapshotData, SyntaxSnapshotId};
use crate::grammar::build::GrammarBuild;
use crate::grammar::event::PendingSyntaxDiagnostic;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::parser::ShadowFragmentKind;

use super::{ParseFailure, ParseStatus};

/// One immutable, source-bound result of the accepted private grammar.
#[derive(Clone, Debug)]
pub(crate) struct BoundParsedSource {
    product: BoundSyntaxProduct,
}

/// Standalone typed fragment attached to its own database-owned syntax lineage.
#[derive(Clone, Debug)]
pub(crate) struct BoundFragment<K> {
    product: BoundSyntaxProduct,
    root: SyntaxNodeHandle,
    span: SourceSpan,
    marker: PhantomData<fn() -> K>,
}

pub(crate) type BoundExpressionFragment = BoundFragment<ExpressionFragment>;
pub(crate) type BoundTypeFragment = BoundFragment<TypeFragment>;
pub(crate) type BoundPatternFragment = BoundFragment<PatternFragment>;
pub(crate) type BoundStatementFragment = BoundFragment<StatementFragment>;

pub(crate) trait BoundFragmentKind {
    const GRAMMAR: ShadowFragmentKind;

    fn accepts(kind: SyntaxKind) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExpressionFragment {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypeFragment {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatternFragment {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StatementFragment {}

#[derive(Clone, Debug)]
struct BoundSyntaxProduct {
    syntax: Arc<SyntaxSnapshotData>,
    diagnostics: Arc<[SyntaxDiagnostic]>,
    status: ParseStatus,
}

/// Recoverable grammar diagnostic bound to one immutable source revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SyntaxDiagnostic {
    code: &'static str,
    primary: SourceSpan,
    related: Option<SourceSpan>,
    message: String,
}

impl BoundParsedSource {
    /// Binds a successfully attached snapshot to the diagnostics produced by
    /// the same grammar event transaction.
    pub(crate) fn try_new(
        syntax: Arc<SyntaxSnapshotData>,
        build: &GrammarBuild,
    ) -> Result<Self, SourceSpanError> {
        Ok(Self {
            product: BoundSyntaxProduct::try_new(syntax, build)?,
        })
    }

    /// Qualified grammar snapshot committed by this parse transaction.
    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.product.snapshot_id()
    }

    /// Exact immutable source document owned by the grammar snapshot.
    pub(crate) fn document(&self) -> &Arc<SourceDocument> {
        self.product.document()
    }

    /// Attached grammar and syntax identity inventory.
    pub(crate) const fn syntax(&self) -> &Arc<SyntaxSnapshotData> {
        self.product.syntax()
    }

    /// Recoverable diagnostics emitted by this exact grammar transaction.
    pub(crate) fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        self.product.diagnostics()
    }

    /// Whether the attached tree contains recovery evidence.
    pub(crate) const fn status(&self) -> ParseStatus {
        self.product.status()
    }
}

impl<K: BoundFragmentKind> BoundFragment<K> {
    pub(crate) fn try_new(
        syntax: Arc<SyntaxSnapshotData>,
        build: &GrammarBuild,
        span: SourceSpan,
    ) -> Result<Self, ParseFailure> {
        span.validate_for(syntax.document())
            .map_err(|_| ParseFailure::SourceMismatch)?;
        let root = syntax
            .root_handle()
            .child(SyntaxRole::Element(0))
            .filter(|node| K::accepts(node.kind()))
            .ok_or(ParseFailure::InternalInvariant)?;
        if !range_within(root.range(), span.range()) {
            return Err(ParseFailure::InternalInvariant);
        }
        let product = BoundSyntaxProduct::try_new(syntax, build)
            .map_err(|_| ParseFailure::InternalInvariant)?;
        if product.diagnostics().iter().any(|diagnostic| {
            !range_within(diagnostic.primary().range(), span.range())
                || diagnostic
                    .related()
                    .is_some_and(|related| !range_within(related.range(), span.range()))
        }) {
            return Err(ParseFailure::InternalInvariant);
        }
        Ok(Self {
            product,
            root,
            span,
            marker: PhantomData,
        })
    }

    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.product.snapshot_id()
    }

    pub(crate) fn document(&self) -> &Arc<SourceDocument> {
        self.product.document()
    }

    pub(crate) const fn syntax(&self) -> &Arc<SyntaxSnapshotData> {
        self.product.syntax()
    }

    pub(crate) fn root(&self) -> &SyntaxNodeHandle {
        &self.root
    }

    pub(crate) const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub(crate) fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        self.product.diagnostics()
    }

    pub(crate) const fn status(&self) -> ParseStatus {
        self.product.status()
    }
}

impl BoundFragmentKind for ExpressionFragment {
    const GRAMMAR: ShadowFragmentKind = ShadowFragmentKind::Expression;

    fn accepts(kind: SyntaxKind) -> bool {
        kind.is_expression()
    }
}

impl BoundFragmentKind for TypeFragment {
    const GRAMMAR: ShadowFragmentKind = ShadowFragmentKind::Type;

    fn accepts(kind: SyntaxKind) -> bool {
        kind.is_type_node()
    }
}

impl BoundFragmentKind for PatternFragment {
    const GRAMMAR: ShadowFragmentKind = ShadowFragmentKind::Pattern;

    fn accepts(kind: SyntaxKind) -> bool {
        kind.is_pattern_node()
    }
}

impl BoundFragmentKind for StatementFragment {
    const GRAMMAR: ShadowFragmentKind = ShadowFragmentKind::Statement;

    fn accepts(kind: SyntaxKind) -> bool {
        kind.is_statement()
    }
}

const fn range_within(candidate: SourceRange, owner: SourceRange) -> bool {
    owner.start() <= candidate.start() && candidate.end() <= owner.end()
}

impl BoundSyntaxProduct {
    fn try_new(
        syntax: Arc<SyntaxSnapshotData>,
        build: &GrammarBuild,
    ) -> Result<Self, SourceSpanError> {
        let diagnostics = build
            .diagnostics()
            .iter()
            .map(|diagnostic| SyntaxDiagnostic::bind(syntax.document(), diagnostic))
            .collect::<Result<Arc<[_]>, _>>()?;
        Ok(Self {
            syntax,
            diagnostics,
            status: if build.has_recovery() {
                ParseStatus::Recovered
            } else {
                ParseStatus::Clean
            },
        })
    }

    fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    fn document(&self) -> &Arc<SourceDocument> {
        self.syntax.document()
    }

    const fn syntax(&self) -> &Arc<SyntaxSnapshotData> {
        &self.syntax
    }

    fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        &self.diagnostics
    }

    const fn status(&self) -> ParseStatus {
        self.status
    }
}

impl SyntaxDiagnostic {
    fn bind(
        document: &SourceDocument,
        diagnostic: &PendingSyntaxDiagnostic,
    ) -> Result<Self, SourceSpanError> {
        Ok(Self {
            code: diagnostic.code(),
            primary: document.span(diagnostic.range())?,
            related: diagnostic
                .related_range()
                .map(|range| document.span(range))
                .transpose()?,
            message: diagnostic.message().to_owned(),
        })
    }

    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) const fn primary(&self) -> &SourceSpan {
        &self.primary
    }

    pub(crate) const fn related(&self) -> Option<&SourceSpan> {
        self.related.as_ref()
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}
