//! Immutable data owned by one source-bound parsed snapshot.

use std::sync::Arc;

use arcweft_source::{SourceDocument, SourceSpan, SourceSpanError};

use crate::attachment::{SyntaxSnapshotData, SyntaxSnapshotId};
use crate::grammar::budget::SyntaxParseStats;
use crate::grammar::build::GrammarBuild;
use crate::grammar::event::PendingSyntaxDiagnostic;

use super::ParseStatus;

/// Data shared by every cheap clone of one accepted parsed source.
#[derive(Clone, Debug)]
pub(crate) struct ParsedSourceData {
    syntax: Arc<SyntaxSnapshotData>,
    document: Arc<SourceDocument>,
    diagnostics: Arc<[SyntaxDiagnostic]>,
    status: ParseStatus,
    stats: SyntaxParseStats,
}

/// Recoverable grammar diagnostic bound to one immutable source revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxDiagnostic {
    code: &'static str,
    primary: SourceSpan,
    related: Option<SourceSpan>,
    message: String,
}

impl ParsedSourceData {
    /// Binds a successfully attached snapshot to the diagnostics produced by
    /// the same grammar event transaction.
    pub(crate) fn try_new(
        syntax: Arc<SyntaxSnapshotData>,
        build: &GrammarBuild,
    ) -> Result<Self, SourceSpanError> {
        let diagnostics = build
            .diagnostics()
            .iter()
            .map(|diagnostic| SyntaxDiagnostic::bind(syntax.document(), diagnostic))
            .collect::<Result<Vec<_>, _>>()?;
        let diagnostics = retain_first_diagnostic_identities(diagnostics);
        let document = Arc::clone(syntax.document());
        let data = Self {
            syntax,
            document,
            diagnostics: diagnostics.into(),
            status: if build.has_recovery() {
                ParseStatus::Recovered
            } else {
                ParseStatus::Clean
            },
            stats: build.stats(),
        };
        debug_assert!(
            data.stats
                .matches_publication(data.document.text().len(), data.diagnostics.len()),
            "validated grammar statistics must match publication",
        );
        Ok(data)
    }

    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    pub(crate) const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub(crate) const fn syntax(&self) -> &Arc<SyntaxSnapshotData> {
        &self.syntax
    }

    pub(crate) fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        &self.diagnostics
    }

    pub(crate) const fn status(&self) -> ParseStatus {
        self.status
    }

    #[cfg(test)]
    pub(crate) const fn stats(&self) -> SyntaxParseStats {
        self.stats
    }
}

/// Removes diagnostics with the same structured identity while retaining the
/// first parser event and its presentation message.
fn retain_first_diagnostic_identities(diagnostics: Vec<SyntaxDiagnostic>) -> Vec<SyntaxDiagnostic> {
    diagnostics
        .into_iter()
        .fold(Vec::new(), |mut unique, item| {
            if !unique
                .iter()
                .any(|existing| item.has_same_deduplication_identity(existing))
            {
                unique.push(item);
            }
            unique
        })
}

impl SyntaxDiagnostic {
    fn has_same_deduplication_identity(&self, other: &Self) -> bool {
        self.code == other.code && self.primary == other.primary && self.related == other.related
    }

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

    /// Stable diagnostic code emitted by the attached grammar transaction.
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// Primary span in the exact immutable source revision.
    pub const fn primary(&self) -> &SourceSpan {
        &self.primary
    }

    /// Optional related span in the same immutable source revision.
    pub const fn related(&self) -> Option<&SourceSpan> {
        self.related.as_ref()
    }

    /// Human-readable diagnostic detail; never an identity key.
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod tests {
    use super::{SyntaxDiagnostic, retain_first_diagnostic_identities};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};
    use std::sync::Arc;

    #[test]
    fn diagnostic_dedup_uses_structured_identity_and_preserves_first_event_order() {
        let name = SourceName::path("diagnostics.arcw");
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(name.display_name()).expect("valid source document ID"),
            name,
            Arc::<str>::from("abc"),
        )
        .expect("test source document");
        let diagnostic =
            |code, range, related: Option<SourceRange>, message: &str| SyntaxDiagnostic {
                code,
                primary: document.span(range).expect("valid diagnostic span"),
                related: related.map(|range| document.span(range).expect("valid related span")),
                message: message.to_owned(),
            };
        let second = diagnostic("E_SECOND", SourceRange::new(2, 3), None, "second");
        let first = diagnostic("E_FIRST", SourceRange::new(0, 1), None, "first");
        let same_position_different_message =
            diagnostic("E_FIRST", SourceRange::new(0, 1), None, "different");
        let distinct_related = diagnostic(
            "E_FIRST",
            SourceRange::new(0, 1),
            Some(SourceRange::new(1, 2)),
            "first with related evidence",
        );

        let retained = retain_first_diagnostic_identities(vec![
            second.clone(),
            first.clone(),
            second,
            same_position_different_message,
            distinct_related.clone(),
            first,
        ]);

        assert_eq!(
            retained,
            vec![
                diagnostic("E_SECOND", SourceRange::new(2, 3), None, "second"),
                diagnostic("E_FIRST", SourceRange::new(0, 1), None, "first"),
                distinct_related,
            ]
        );
    }
}
