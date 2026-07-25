//! Byte-exact indentation ownership for colon dialogue applications.

use super::{
    DialogueSurfaceInvariantError, checked_width, source_slice, validate_contains,
    validate_horizontal_whitespace, validate_offset, validate_order,
};
use crate::{ast::common::TextRange, attachment::SyntaxNodeId};
use arcweft_source::SourceDocument;

/// Exact parser classification of content following a colon application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum DialogueIndentation {
    Inline(DialogueInlineIndentation),
    Indented(DialogueIndentedIndentation),
    Missing(DialogueMissingIndentation),
}

/// Source-byte indentation width; tabs intentionally count as one byte.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) struct DialogueIndentationBytes(usize);

/// Exact authored horizontal-whitespace prefix at one physical line start.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) struct DialogueIndentationPrefix {
    range: TextRange,
    width: DialogueIndentationBytes,
}

/// One accepted physical LF or CRLF line ending.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) struct DialogueLineEnding {
    range: TextRange,
    kind: DialogueLineEndingKind,
}

/// Authored line-ending byte sequence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum DialogueLineEndingKind {
    Lf,
    CrLf,
}

/// Inline content positioning retained separately from semantic content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct DialogueInlineIndentation {
    head: DialogueIndentationPrefix,
    separator: TextRange,
    boundary: DialogueInlineBoundary,
}

/// Exact grammar boundary that ended inline content scanning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum DialogueInlineBoundary {
    LineEnding(DialogueLineEnding),
    AttachedPlan {
        plan_syntax: SyntaxNodeId,
        at: usize,
    },
    OwnerEnd {
        anchor: usize,
    },
    EndOfDocument {
        anchor: usize,
    },
}

/// Raw indented body plus its dedent and recovery facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct DialogueIndentedIndentation {
    head: DialogueIndentationPrefix,
    head_line_ending: DialogueLineEnding,
    body: TextRange,
    base: DialogueIndentationPrefix,
    dedent: DialogueDedentBoundary,
    issues: Box<[DialogueIndentationIssue]>,
}

/// Empty/missing content source ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct DialogueMissingIndentation {
    head: DialogueIndentationPrefix,
    after_colon: DialogueMissingAfterColon,
    retained_trivia: Option<TextRange>,
    insertion: usize,
    boundary: DialogueMissingBoundary,
}

/// Trivia shape immediately following a colon whose content is missing.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum DialogueMissingAfterColon {
    SameLine {
        separator: TextRange,
    },
    NextLine {
        head_line_ending: DialogueLineEnding,
    },
}

/// Boundary family selected for missing inline or indented content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum DialogueMissingBoundary {
    Inline(DialogueInlineBoundary),
    Indented(DialogueDedentBoundary),
}

/// Exact source boundary that ended an indented dialogue body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum DialogueDedentBoundary {
    DedentedLine {
        line_start: usize,
        indentation: DialogueIndentationPrefix,
    },
    AttachedPlan {
        plan_syntax: SyntaxNodeId,
        line_start: usize,
        indentation: DialogueIndentationPrefix,
    },
    OwnerEnd {
        anchor: usize,
    },
    EndOfDocument {
        anchor: usize,
    },
}

/// Recoverable indentation defect retained without normalizing source bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum DialogueIndentationIssue {
    Misaligned {
        indentation: DialogueIndentationPrefix,
        required: DialogueIndentationBytes,
    },
}

impl DialogueIndentationBytes {
    pub const fn get(self) -> usize {
        self.0
    }
}

impl DialogueIndentationPrefix {
    pub(in super::super) fn try_new(
        document: &SourceDocument,
        range: TextRange,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        validate_horizontal_whitespace(document, range)?;
        if range.start() > 0 && document.text().as_bytes()[range.start() - 1] != b'\n' {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        let width = checked_width(range)?;
        Ok(Self {
            range,
            width: DialogueIndentationBytes(width),
        })
    }

    pub const fn range(self) -> TextRange {
        self.range
    }

    pub const fn width(self) -> DialogueIndentationBytes {
        self.width
    }
}

impl DialogueLineEnding {
    pub(in super::super) fn try_new(
        document: &SourceDocument,
        range: TextRange,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        let kind = match source_slice(document, range)?.as_bytes() {
            b"\n" => DialogueLineEndingKind::Lf,
            b"\r\n" => DialogueLineEndingKind::CrLf,
            _ => {
                return Err(DialogueSurfaceInvariantError::InvalidLineEndingBytes { range });
            }
        };
        Ok(Self { range, kind })
    }

    pub const fn range(self) -> TextRange {
        self.range
    }

    pub const fn kind(self) -> DialogueLineEndingKind {
        self.kind
    }
}

impl DialogueIndentation {
    pub(in super::super) fn try_inline(
        document: &SourceDocument,
        head: DialogueIndentationPrefix,
        separator: TextRange,
        boundary: DialogueInlineBoundary,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        validate_prefix(document, head)?;
        validate_horizontal_whitespace(document, separator)?;
        validate_order(head.range(), separator)?;
        let boundary_start = validate_inline_boundary(document, &boundary)?;
        if separator.end() > boundary_start {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        Ok(Self::Inline(DialogueInlineIndentation {
            head,
            separator,
            boundary,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn try_indented(
        document: &SourceDocument,
        head: DialogueIndentationPrefix,
        head_line_ending: DialogueLineEnding,
        body: TextRange,
        base: DialogueIndentationPrefix,
        dedent: DialogueDedentBoundary,
        issues: Box<[DialogueIndentationIssue]>,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        validate_prefix(document, head)?;
        validate_line_ending(document, head_line_ending)?;
        source_slice(document, body)?;
        validate_prefix(document, base)?;
        validate_order(head.range(), head_line_ending.range())?;
        if body.start() != head_line_ending.range().end() {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        validate_contains(body, base.range())?;
        if base.width().get() <= head.width().get() {
            return Err(DialogueSurfaceInvariantError::InvalidBaseRelation);
        }
        if validate_dedent_boundary(document, head, &dedent)? != body.end() {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        for issue in &issues {
            match *issue {
                DialogueIndentationIssue::Misaligned {
                    indentation,
                    required,
                } => {
                    validate_prefix(document, indentation)?;
                    validate_contains(body, indentation.range())?;
                    if required != base.width()
                        || indentation.width().get() <= head.width().get()
                        || indentation.width().get() >= base.width().get()
                    {
                        return Err(DialogueSurfaceInvariantError::IssueOutsideBody);
                    }
                }
            }
        }
        Ok(Self::Indented(DialogueIndentedIndentation {
            head,
            head_line_ending,
            body,
            base,
            dedent,
            issues,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn try_missing(
        document: &SourceDocument,
        head: DialogueIndentationPrefix,
        after_colon: DialogueMissingAfterColon,
        retained_trivia: Option<TextRange>,
        insertion: usize,
        boundary: DialogueMissingBoundary,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        validate_prefix(document, head)?;
        let after_colon_end = match after_colon {
            DialogueMissingAfterColon::SameLine { separator } => {
                validate_horizontal_whitespace(document, separator)?;
                separator.end()
            }
            DialogueMissingAfterColon::NextLine { head_line_ending } => {
                validate_line_ending(document, head_line_ending)?;
                head_line_ending.range().end()
            }
        };
        validate_offset(document, insertion)?;
        let boundary_start = match &boundary {
            DialogueMissingBoundary::Inline(boundary) => {
                validate_inline_boundary(document, boundary)?
            }
            DialogueMissingBoundary::Indented(boundary) => {
                validate_dedent_boundary(document, head, boundary)?
            }
        };
        if insertion < after_colon_end || insertion > boundary_start {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        if let Some(trivia) = retained_trivia {
            source_slice(document, trivia)?;
            if trivia.start() < after_colon_end || trivia.end() > boundary_start {
                return Err(DialogueSurfaceInvariantError::Containment {
                    container: TextRange::new(after_colon_end, boundary_start),
                    child: trivia,
                });
            }
        }
        Ok(Self::Missing(DialogueMissingIndentation {
            head,
            after_colon,
            retained_trivia,
            insertion,
            boundary,
        }))
    }
}

impl DialogueInlineIndentation {
    pub const fn head(&self) -> DialogueIndentationPrefix {
        self.head
    }

    pub const fn separator(&self) -> TextRange {
        self.separator
    }

    pub const fn boundary(&self) -> &DialogueInlineBoundary {
        &self.boundary
    }
}

impl DialogueInlineBoundary {
    pub const fn start(&self) -> usize {
        match self {
            Self::LineEnding(line_ending) => line_ending.range().start(),
            Self::AttachedPlan { at, .. } => *at,
            Self::OwnerEnd { anchor } | Self::EndOfDocument { anchor } => *anchor,
        }
    }

    pub const fn plan_syntax(&self) -> Option<SyntaxNodeId> {
        match self {
            Self::AttachedPlan { plan_syntax, .. } => Some(*plan_syntax),
            Self::LineEnding(_) | Self::OwnerEnd { .. } | Self::EndOfDocument { .. } => None,
        }
    }
}

impl DialogueIndentedIndentation {
    pub const fn head(&self) -> DialogueIndentationPrefix {
        self.head
    }

    pub const fn head_line_ending(&self) -> DialogueLineEnding {
        self.head_line_ending
    }

    pub const fn body(&self) -> TextRange {
        self.body
    }

    pub const fn base(&self) -> DialogueIndentationPrefix {
        self.base
    }

    pub const fn dedent(&self) -> &DialogueDedentBoundary {
        &self.dedent
    }

    pub const fn issues(&self) -> &[DialogueIndentationIssue] {
        &self.issues
    }
}

impl DialogueMissingIndentation {
    pub const fn head(&self) -> DialogueIndentationPrefix {
        self.head
    }

    pub const fn after_colon(&self) -> DialogueMissingAfterColon {
        self.after_colon
    }

    pub const fn retained_trivia(&self) -> Option<TextRange> {
        self.retained_trivia
    }

    pub const fn insertion(&self) -> usize {
        self.insertion
    }

    pub const fn boundary(&self) -> &DialogueMissingBoundary {
        &self.boundary
    }
}

impl DialogueDedentBoundary {
    pub const fn start(&self) -> usize {
        match self {
            Self::DedentedLine { line_start, .. } | Self::AttachedPlan { line_start, .. } => {
                *line_start
            }
            Self::OwnerEnd { anchor } | Self::EndOfDocument { anchor } => *anchor,
        }
    }

    pub const fn indentation(&self) -> Option<DialogueIndentationPrefix> {
        match self {
            Self::DedentedLine { indentation, .. } | Self::AttachedPlan { indentation, .. } => {
                Some(*indentation)
            }
            Self::OwnerEnd { .. } | Self::EndOfDocument { .. } => None,
        }
    }

    pub const fn plan_syntax(&self) -> Option<SyntaxNodeId> {
        match self {
            Self::AttachedPlan { plan_syntax, .. } => Some(*plan_syntax),
            Self::DedentedLine { .. } | Self::OwnerEnd { .. } | Self::EndOfDocument { .. } => None,
        }
    }
}

impl DialogueIndentationIssue {
    pub const fn indentation(self) -> DialogueIndentationPrefix {
        match self {
            Self::Misaligned { indentation, .. } => indentation,
        }
    }

    pub const fn required(self) -> DialogueIndentationBytes {
        match self {
            Self::Misaligned { required, .. } => required,
        }
    }
}

fn validate_prefix(
    document: &SourceDocument,
    prefix: DialogueIndentationPrefix,
) -> Result<(), DialogueSurfaceInvariantError> {
    let observed = DialogueIndentationPrefix::try_new(document, prefix.range())?;
    if observed.width() == prefix.width() {
        Ok(())
    } else {
        Err(DialogueSurfaceInvariantError::WidthMismatch {
            expected: prefix.width().get(),
            actual: observed.width().get(),
        })
    }
}

fn validate_line_ending(
    document: &SourceDocument,
    line_ending: DialogueLineEnding,
) -> Result<(), DialogueSurfaceInvariantError> {
    let observed = DialogueLineEnding::try_new(document, line_ending.range())?;
    if observed.kind() == line_ending.kind() {
        Ok(())
    } else {
        Err(DialogueSurfaceInvariantError::InvalidLineEndingBytes {
            range: line_ending.range(),
        })
    }
}

fn validate_inline_boundary(
    document: &SourceDocument,
    boundary: &DialogueInlineBoundary,
) -> Result<usize, DialogueSurfaceInvariantError> {
    match boundary {
        DialogueInlineBoundary::LineEnding(line_ending) => {
            validate_line_ending(document, *line_ending)?;
            Ok(line_ending.range().start())
        }
        DialogueInlineBoundary::AttachedPlan { at, .. }
        | DialogueInlineBoundary::OwnerEnd { anchor: at } => {
            validate_offset(document, *at)?;
            Ok(*at)
        }
        DialogueInlineBoundary::EndOfDocument { anchor } => {
            validate_offset(document, *anchor)?;
            if *anchor == document.text().len() {
                Ok(*anchor)
            } else {
                Err(DialogueSurfaceInvariantError::BoundaryMismatch)
            }
        }
    }
}

fn validate_dedent_boundary(
    document: &SourceDocument,
    head: DialogueIndentationPrefix,
    boundary: &DialogueDedentBoundary,
) -> Result<usize, DialogueSurfaceInvariantError> {
    match boundary {
        DialogueDedentBoundary::DedentedLine {
            line_start,
            indentation,
        } => {
            validate_offset(document, *line_start)?;
            validate_prefix(document, *indentation)?;
            if indentation.range().start() != *line_start
                || indentation.width().get() > head.width().get()
            {
                return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
            }
            Ok(*line_start)
        }
        DialogueDedentBoundary::AttachedPlan {
            line_start,
            indentation,
            ..
        } => {
            validate_offset(document, *line_start)?;
            validate_prefix(document, *indentation)?;
            if indentation.range().start() != *line_start
                || indentation.width() != head.width()
                || source_slice(document, indentation.range())?
                    != source_slice(document, head.range())?
            {
                return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
            }
            Ok(*line_start)
        }
        DialogueDedentBoundary::OwnerEnd { anchor } => {
            validate_offset(document, *anchor)?;
            Ok(*anchor)
        }
        DialogueDedentBoundary::EndOfDocument { anchor } => {
            validate_offset(document, *anchor)?;
            if *anchor == document.text().len() {
                Ok(*anchor)
            } else {
                Err(DialogueSurfaceInvariantError::BoundaryMismatch)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DialogueDedentBoundary, DialogueIndentation, DialogueIndentationBytes,
        DialogueIndentationIssue, DialogueIndentationPrefix, DialogueLineEnding,
        DialogueLineEndingKind,
    };
    use crate::{
        ast::common::TextRange, expr::dialogue_application::DialogueSurfaceInvariantError,
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    fn document(text: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/dialogue-indentation-test").expect("document ID"),
            SourceName::Generated,
            text,
        )
        .expect("test document")
    }

    #[test]
    fn indentation_width_is_exact_source_bytes() {
        let document = document(" \t語り手:\r\n \t  text\n");
        let prefix =
            DialogueIndentationPrefix::try_new(&document, TextRange::new(0, 2)).expect("prefix");
        let ending = DialogueLineEnding::try_new(&document, TextRange::new(12, 14)).expect("CRLF");

        assert_eq!(prefix.width().get(), 2);
        assert_eq!(ending.kind(), DialogueLineEndingKind::CrLf);
    }

    #[test]
    fn indentation_rejects_non_ascii_whitespace_and_bare_cr() {
        let document = document("\u{00a0}\r");
        assert!(matches!(
            DialogueIndentationPrefix::try_new(&document, TextRange::new(0, 2)),
            Err(DialogueSurfaceInvariantError::NonWhitespacePrefix { .. })
        ));
        assert!(matches!(
            DialogueLineEnding::try_new(&document, TextRange::new(2, 3)),
            Err(DialogueSurfaceInvariantError::InvalidLineEndingBytes { .. })
        ));
    }

    #[test]
    fn indented_body_retains_misalignment_without_normalizing_it() {
        let document = document(" a:\n    first\n  second\nnext");
        let head =
            DialogueIndentationPrefix::try_new(&document, TextRange::new(0, 1)).expect("head");
        let ending =
            DialogueLineEnding::try_new(&document, TextRange::new(3, 4)).expect("line ending");
        let base =
            DialogueIndentationPrefix::try_new(&document, TextRange::new(4, 8)).expect("base");
        let misaligned =
            DialogueIndentationPrefix::try_new(&document, TextRange::new(14, 16)).expect("issue");
        let body = TextRange::new(4, 23);

        let indentation = DialogueIndentation::try_indented(
            &document,
            head,
            ending,
            body,
            base,
            DialogueDedentBoundary::DedentedLine {
                line_start: 23,
                indentation: DialogueIndentationPrefix::try_new(&document, TextRange::new(23, 23))
                    .expect("dedent"),
            },
            Box::new([DialogueIndentationIssue::Misaligned {
                indentation: misaligned,
                required: DialogueIndentationBytes(4),
            }]),
        )
        .expect("indented dialogue");

        let DialogueIndentation::Indented(indentation) = indentation else {
            panic!("expected indented form");
        };
        assert_eq!(indentation.body(), body);
        assert_eq!(indentation.issues().len(), 1);
    }
}
