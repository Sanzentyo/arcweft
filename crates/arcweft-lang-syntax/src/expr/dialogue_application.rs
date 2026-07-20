//! Private substrate for generic postfix-bracket and colon dialogue applications.
//!
//! The module remains private until the direct public CST/AST replacement is
//! ready to migrate every exhaustive consumer in one compiling series.

#![allow(dead_code, unused_imports)]

use crate::ast::common::TextRange;
use arcweft_source::SourceDocument;
use thiserror::Error;

mod candidate;
mod indentation;
mod surface;

pub(super) use candidate::{
    ApplicationRecoveryStatus, PostfixBracketCandidates, PostfixCandidateFailure,
    PostfixCandidateFailureKind, PostfixCandidateFailureSite, PostfixCandidateInvariantError,
    PostfixDialogueCandidate, PostfixIndexCandidate,
};
pub(super) use indentation::{
    DialogueDedentBoundary, DialogueIndentation, DialogueIndentationBytes,
    DialogueIndentationIssue, DialogueIndentationPrefix, DialogueIndentedIndentation,
    DialogueInlineBoundary, DialogueInlineIndentation, DialogueLineEnding, DialogueLineEndingKind,
    DialogueMissingAfterColon, DialogueMissingBoundary, DialogueMissingIndentation,
};
pub(super) use surface::{
    AttachedLinePlanSurface, BracketDialogueApplicationSurface, BracketTerminatorSyntax,
    ColonDialogueApplicationSurface, DialogueContentApplicationSurface,
    DialogueContentRecoveryBoundarySyntax, DialogueContentSite, PostfixBracketBoundaryToken,
    PostfixBracketRecoveryBoundarySyntax, PostfixBracketSurface,
};
/// An impossible relation detected while assembling parser-owned dialogue surfaces.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(super) enum DialogueSurfaceInvariantError {
    #[error("checked source arithmetic overflowed")]
    ArithmeticOverflow,
    #[error("range {range:?} is reversed")]
    ReversedRange { range: TextRange },
    #[error("range {range:?} is outside the {document_len}-byte source document")]
    OutOfDocumentRange {
        range: TextRange,
        document_len: usize,
    },
    #[error("source offset {offset} is outside the {document_len}-byte source document")]
    OutOfDocumentOffset { offset: usize, document_len: usize },
    #[error("source offset {offset} is not a UTF-8 boundary")]
    NonUtf8Boundary { offset: usize },
    #[error("indentation range {range:?} contains a non-horizontal-whitespace byte")]
    NonWhitespacePrefix { range: TextRange },
    #[error("indentation width mismatch: expected {expected}, observed {actual}")]
    WidthMismatch { expected: usize, actual: usize },
    #[error("range {range:?} is not exactly LF or CRLF")]
    InvalidLineEndingBytes { range: TextRange },
    #[error("range {range:?} is not the required `{expected}` delimiter")]
    InvalidDelimiter { expected: char, range: TextRange },
    #[error("source ranges are not in grammar order: {earlier:?} before {later:?}")]
    Ordering {
        earlier: TextRange,
        later: TextRange,
    },
    #[error("range {child:?} is not contained by {container:?}")]
    Containment {
        container: TextRange,
        child: TextRange,
    },
    #[error("the indented dialogue base is not deeper than its head")]
    InvalidBaseRelation,
    #[error("an indentation issue lies outside the retained body")]
    IssueOutsideBody,
    #[error("a retained boundary does not match the adjacent source position")]
    BoundaryMismatch,
    #[error("syntax identities assigned to distinct child roles must differ")]
    DuplicateSyntaxIdentity,
    #[error("dialogue content and indentation recovery disagree")]
    ContentIndentationMismatch,
    #[error("an attached line plan precedes the application terminator")]
    PlanOrdering,
}

fn source_slice(
    document: &SourceDocument,
    range: TextRange,
) -> Result<&str, DialogueSurfaceInvariantError> {
    if range.start() > range.end() {
        return Err(DialogueSurfaceInvariantError::ReversedRange { range });
    }
    let document_len = document.text().len();
    if range.end() > document_len {
        return Err(DialogueSurfaceInvariantError::OutOfDocumentRange {
            range,
            document_len,
        });
    }
    validate_offset(document, range.start())?;
    validate_offset(document, range.end())?;
    Ok(&document.text()[range.as_range()])
}

fn validate_offset(
    document: &SourceDocument,
    offset: usize,
) -> Result<(), DialogueSurfaceInvariantError> {
    let document_len = document.text().len();
    if offset > document_len {
        return Err(DialogueSurfaceInvariantError::OutOfDocumentOffset {
            offset,
            document_len,
        });
    }
    if !document.text().is_char_boundary(offset) {
        return Err(DialogueSurfaceInvariantError::NonUtf8Boundary { offset });
    }
    Ok(())
}

fn validate_horizontal_whitespace(
    document: &SourceDocument,
    range: TextRange,
) -> Result<(), DialogueSurfaceInvariantError> {
    if source_slice(document, range)?
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'))
    {
        Ok(())
    } else {
        Err(DialogueSurfaceInvariantError::NonWhitespacePrefix { range })
    }
}

fn validate_delimiter(
    document: &SourceDocument,
    range: TextRange,
    expected: u8,
) -> Result<(), DialogueSurfaceInvariantError> {
    let source = source_slice(document, range)?;
    if source.as_bytes() == [expected] {
        Ok(())
    } else {
        Err(DialogueSurfaceInvariantError::InvalidDelimiter {
            expected: char::from(expected),
            range,
        })
    }
}

fn validate_order(
    earlier: TextRange,
    later: TextRange,
) -> Result<(), DialogueSurfaceInvariantError> {
    if earlier.end() <= later.start() {
        Ok(())
    } else {
        Err(DialogueSurfaceInvariantError::Ordering { earlier, later })
    }
}

fn validate_contains(
    container: TextRange,
    child: TextRange,
) -> Result<(), DialogueSurfaceInvariantError> {
    if container.start() <= child.start() && child.end() <= container.end() {
        Ok(())
    } else {
        Err(DialogueSurfaceInvariantError::Containment { container, child })
    }
}

fn checked_width(range: TextRange) -> Result<usize, DialogueSurfaceInvariantError> {
    range
        .end()
        .checked_sub(range.start())
        .ok_or(DialogueSurfaceInvariantError::ArithmeticOverflow)
}
