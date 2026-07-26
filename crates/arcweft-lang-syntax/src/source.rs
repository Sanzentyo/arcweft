//! Parsed source container and line indexing.

use crate::ast::common::TextRange;
use crate::ast::items::TypedSyntaxTree;
use crate::cst::{SyntaxNode, SyntaxParseStats};
use crate::parser::recovery::{ParseError, ParseErrorKind, RecoveryEdit, RecoverySuggestion};
use arcweft_source::{
    SourceDocument, SourceDocumentIdentity, SourceRange, SourceSpan, SourceSpanError,
};
use std::{cmp::Ordering, sync::Arc};

/// Fully parsed source file.
///
/// The lossless syntax tree is always present. `errors` records recoverable
/// syntax failures, while `typed_tree` preserves the current semantic view used
/// by HIR and checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedSource {
    document: Arc<SourceDocument>,
    syntax: SyntaxNode,
    typed_tree: TypedSyntaxTree,
    errors: Vec<ParseError>,
    syntax_stats: SyntaxParseStats,
    line_index: LineIndex,
}

/// Byte offsets of line starts for source-coordinate conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineIndex {
    starts: Vec<usize>,
}

impl ParsedSource {
    pub(crate) fn new(
        document: Arc<SourceDocument>,
        syntax: SyntaxNode,
        typed_tree: TypedSyntaxTree,
        mut errors: Vec<ParseError>,
        mut syntax_stats: SyntaxParseStats,
    ) -> Self {
        normalize_parse_errors(&mut errors);
        let prefix_depth_failures = errors
            .iter()
            .filter(|error| error.kind() == ParseErrorKind::ExpressionPrefixDepthLimit)
            .count();
        syntax_stats.checked_add_prefix_depth_limit_failures(prefix_depth_failures);
        let line_index = LineIndex::new(document.text());
        Self {
            document,
            syntax,
            typed_tree,
            errors,
            syntax_stats,
            line_index,
        }
    }

    /// Original source text.
    pub fn source(&self) -> &str {
        self.document.text()
    }

    /// Immutable source document that owns this parse revision.
    pub const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    /// Exact revision-bound identity shared by all spans produced from this parse.
    pub fn identity(&self) -> &SourceDocumentIdentity {
        self.document.identity()
    }

    /// Binds a parser byte range to the exact source revision.
    pub fn span(&self, range: TextRange) -> Result<SourceSpan, SourceSpanError> {
        self.document
            .span(SourceRange::new(range.start(), range.end()))
    }

    /// Lossless rowan syntax tree.
    pub const fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// Typed syntax model used by current HIR lowering.
    pub const fn typed_tree(&self) -> &TypedSyntaxTree {
        &self.typed_tree
    }

    /// Recoverable parse diagnostics.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Path-free counters collected by the syntax parser.
    pub const fn syntax_stats(&self) -> SyntaxParseStats {
        self.syntax_stats
    }

    /// Line index for byte-offset diagnostics.
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// True when no parse diagnostics were emitted.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Consumes the parsed source and returns the typed syntax model.
    pub fn into_typed_tree(self) -> TypedSyntaxTree {
        self.typed_tree
    }
}

fn normalize_parse_errors(errors: &mut Vec<ParseError>) {
    errors.sort_by(compare_parse_errors);
    errors.dedup();
}

fn compare_parse_errors(left: &ParseError, right: &ParseError) -> Ordering {
    left.range()
        .start()
        .cmp(&right.range().start())
        .then_with(|| left.range().end().cmp(&right.range().end()))
        .then_with(|| left.code().cmp(right.code()))
        .then_with(|| left.message().cmp(right.message()))
        .then_with(|| left.expected().cmp(right.expected()))
        .then_with(|| left.found().cmp(&right.found()))
        .then_with(|| compare_recovery(left.recovery(), right.recovery()))
}

fn compare_recovery(left: &[RecoverySuggestion], right: &[RecoverySuggestion]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let ordering = left
            .message()
            .cmp(right.message())
            .then_with(|| left.applicability().cmp(&right.applicability()))
            .then_with(|| compare_recovery_edits(left.edits(), right.edits()));
        if !ordering.is_eq() {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_recovery_edits(left: &[RecoveryEdit], right: &[RecoveryEdit]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let ordering = left
            .range()
            .start()
            .cmp(&right.range().start())
            .then_with(|| left.range().end().cmp(&right.range().end()))
            .then_with(|| left.replacement().cmp(right.replacement()));
        if !ordering.is_eq() {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        for (index, ch) in source.char_indices() {
            if ch == '\n' {
                starts.push(index + ch.len_utf8());
            }
        }
        Self { starts }
    }

    /// Start byte offset for each line.
    pub fn starts(&self) -> &[usize] {
        &self.starts
    }

    /// Converts a byte offset to zero-based line and column.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let line = self.starts.partition_point(|start| *start <= offset);
        let line = line.saturating_sub(1);
        (line, offset.saturating_sub(self.starts[line]))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_parse_errors;
    use crate::ast::common::TextRange;
    use crate::parser::recovery::{ParseError, ParseErrorKind, RecoveryEdit, RecoverySuggestion};
    use arcweft_source::DiagnosticApplicability;

    #[test]
    fn parse_errors_are_sorted_and_exact_duplicates_are_removed() {
        let duplicate = ParseError::new_with_kind(
            ParseErrorKind::Generic,
            TextRange::new(2, 4),
            Vec::new(),
            None,
            "generic".to_owned(),
            Vec::new(),
        );
        let mut errors = vec![
            ParseError::new_with_kind(
                ParseErrorKind::ViewPartInvalidLocalName,
                TextRange::new(2, 4),
                Vec::new(),
                None,
                "view".to_owned(),
                Vec::new(),
            ),
            duplicate.clone(),
            ParseError::new_with_kind(
                ParseErrorKind::AssertionInvalidArgument,
                TextRange::new(2, 4),
                Vec::new(),
                None,
                "assertion".to_owned(),
                Vec::new(),
            ),
            duplicate,
        ];

        normalize_parse_errors(&mut errors);

        assert_eq!(errors.len(), 3);
        assert_eq!(errors[0].kind(), ParseErrorKind::AssertionInvalidArgument);
        assert_eq!(errors[1].kind(), ParseErrorKind::Generic);
        assert_eq!(errors[2].kind(), ParseErrorKind::ViewPartInvalidLocalName);
    }

    #[test]
    fn parse_errors_with_different_recovery_evidence_are_not_deduplicated() {
        let plain = ParseError::new_with_kind(
            ParseErrorKind::Generic,
            TextRange::new(2, 4),
            vec!["value".to_owned()],
            Some("token".to_owned()),
            "same message".to_owned(),
            Vec::new(),
        );
        let recovered = ParseError::new_with_kind(
            ParseErrorKind::Generic,
            TextRange::new(2, 4),
            vec!["value".to_owned()],
            Some("token".to_owned()),
            "same message".to_owned(),
            vec![
                RecoverySuggestion::new("replace token")
                    .with_edit(RecoveryEdit::new(TextRange::new(2, 4), "value"))
                    .with_applicability(DiagnosticApplicability::MachineApplicable),
            ],
        );
        let mut errors = vec![recovered.clone(), plain.clone(), recovered];

        normalize_parse_errors(&mut errors);

        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0], plain);
        assert_eq!(errors[1].recovery().len(), 1);
    }
}
