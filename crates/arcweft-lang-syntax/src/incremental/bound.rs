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

use std::sync::Arc;

use arcweft_source::{SourceDocument, SourceSpan, SourceSpanError};

use crate::attachment::{SyntaxNodeHandle, SyntaxSnapshotData, SyntaxSnapshotId};
use crate::grammar::build::GrammarBuild;
use crate::grammar::event::PendingSyntaxDiagnostic;
use crate::grammar::kinds::SyntaxRole;

use super::{ParseFailure, ParseStatus};

/// One immutable, source-bound result of the accepted private grammar.
#[derive(Clone, Debug)]
pub(crate) struct BoundParsedSource {
    product: BoundSyntaxProduct,
}

/// Standalone expression attached to its own database-owned syntax lineage.
#[derive(Clone, Debug)]
pub(crate) struct BoundExpressionFragment {
    product: BoundSyntaxProduct,
    root: SyntaxNodeHandle,
}

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

impl BoundExpressionFragment {
    pub(crate) fn try_new(
        syntax: Arc<SyntaxSnapshotData>,
        build: &GrammarBuild,
    ) -> Result<Self, ParseFailure> {
        let root = syntax
            .root_handle()
            .child(SyntaxRole::Element(0))
            .filter(|node| node.kind().is_expression())
            .ok_or(ParseFailure::InternalInvariant)?;
        let product = BoundSyntaxProduct::try_new(syntax, build)
            .map_err(|_| ParseFailure::InternalInvariant)?;
        Ok(Self { product, root })
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

    pub(crate) fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        self.product.diagnostics()
    }

    pub(crate) const fn status(&self) -> ParseStatus {
        self.product.status()
    }
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
