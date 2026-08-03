//! Shared lossless cursor scans, delimiters, and missing-token recovery.

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use crate::grammar::event::{ExpectedToken, PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_open_delimiter(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    spelling: &str,
) {
    parser.start(kind, SyntaxRole::OpenDelimiter);
    debug_assert!(parser.at(spelling));
    parser.bump();
    parser.finish();
}

pub(super) fn emit_close_delimiter(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    spelling: &'static str,
    diagnostic: &'static str,
) {
    parser.start(kind, SyntaxRole::CloseDelimiter);
    if parser.at(spelling) {
        parser.bump();
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: ExpectedToken::try_with_spelling(SyntaxKind::PunctuationToken, spelling)
                .expect("real grammar punctuation token"),
            at,
        });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic,
            SourceRange::new(at, at),
            format!("missing closing `{spelling}`"),
        )));
    }
    parser.finish();
}

pub(super) fn emit_missing_delimiter(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(kind, role);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at: parser.current_offset(),
    });
    parser.finish();
}

/// Emits one required punctuation owner as either authored bytes or the exact
/// parser-selected insertion site.  The caller supplies the domain role, so
/// attached consumers never need to search source text for the token.
pub(super) fn emit_required_punctuation(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
    spelling: &'static str,
    diagnostic: &'static str,
    message: &'static str,
) -> bool {
    parser.start(kind, role);
    let authored = if parser.at(spelling) {
        parser.bump();
        true
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: ExpectedToken::try_with_spelling(SyntaxKind::PunctuationToken, spelling)
                .expect("real grammar punctuation token"),
            at,
        });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic,
            SourceRange::new(at, at),
            message,
        )));
        false
    };
    parser.finish();
    authored
}

/// Emits one required keyword owner as authored bytes or its exact insertion.
pub(super) fn emit_required_keyword(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
    spelling: &'static str,
    diagnostic: &'static str,
    message: &'static str,
) -> bool {
    parser.start(kind, role);
    let authored = if parser.at(spelling) {
        parser.bump();
        true
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: ExpectedToken::try_with_spelling(SyntaxKind::KeywordToken, spelling)
                .expect("real grammar keyword token"),
            at,
        });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic,
            SourceRange::new(at, at),
            message,
        )));
        false
    };
    parser.finish();
    authored
}

pub(super) fn expected(kind: SyntaxKind) -> ExpectedToken {
    ExpectedToken::try_new(kind).expect("real grammar token kind")
}

pub(super) fn find_header_boundary(parser: &ShadowDocumentParser<'_, '_>, start: usize) -> usize {
    find_top_level_boundary(
        parser,
        start,
        &[
            "where",
            "requires",
            "ensures",
            "invariant",
            "assume",
            "reads",
            "effects",
            "modifies",
            "decreases",
            "=",
            "{",
        ],
    )
}

pub(super) fn find_top_level_boundary(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    boundaries: &[&str],
) -> usize {
    let mut depth = 0_usize;
    let mut index = start;
    while let Some(token) = parser.token_at(index) {
        let text = parser.text_of(token);
        if depth == 0 && boundaries.contains(&text) {
            return index;
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }
    index
}

pub(super) fn find_matching_close(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    opening: &str,
) -> Option<usize> {
    find_matching_close_before(parser, start, token_count(parser), opening)
}

/// Finds the close paired with an already-consumed opening delimiter without
/// crossing a caller-selected grammar recovery boundary.
pub(super) fn find_matching_close_before(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    opening: &str,
) -> Option<usize> {
    let (open, close) = match opening {
        "(" => ("(", ")"),
        "[" => ("[", "]"),
        "{" => ("{", "}"),
        "<" => ("<", ">"),
        _ => return None,
    };
    let mut depth = 0_usize;
    for index in start..end {
        let token = parser.token_at(index)?;
        match parser.text_of(token) {
            text if text == open => depth += 1,
            text if text == close && depth == 0 => return Some(index),
            text if text == close => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

pub(super) fn find_statement_terminator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<(usize, bool)> {
    let mut delimiters = Vec::<&str>::new();
    for index in start..end {
        let token = parser.token_at(index)?;
        let text = parser.text_of(token);
        if delimiters.is_empty() && (text == ";" || token.kind() == SyntaxKind::NewlineToken) {
            return Some((index, text == ";"));
        }
        match text {
            "(" | "[" | "{" => delimiters.push(text),
            ")" if delimiters.last() == Some(&"(") => {
                delimiters.pop();
            }
            "]" if delimiters.last() == Some(&"[") => {
                delimiters.pop();
            }
            "}" if delimiters.last() == Some(&"{") => {
                delimiters.pop();
            }
            _ => {}
        }
    }
    None
}

pub(super) fn first_significant(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    (start..end).find(|index| {
        parser
            .token_at(*index)
            .is_some_and(|token| !is_trivia(token.kind()))
    })
}

pub(super) fn trimmed_end(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> usize {
    (start..end)
        .rev()
        .find(|index| {
            parser
                .token_at(*index)
                .is_some_and(|token| !is_trivia(token.kind()))
        })
        .map_or(start, |index| index + 1)
}

pub(super) fn token_count(parser: &ShadowDocumentParser<'_, '_>) -> usize {
    let mut index = parser.cursor();
    while parser.token_at(index).is_some() {
        index += 1;
    }
    index
}

pub(super) fn token_text<'a>(
    parser: &'a ShadowDocumentParser<'_, '_>,
    index: usize,
) -> Option<&'a str> {
    parser.token_at(index).map(|token| parser.text_of(token))
}

pub(super) fn bump_until(parser: &mut ShadowDocumentParser<'_, '_>, exclusive: usize) {
    while parser.cursor() < exclusive && parser.bump().is_some() {}
}

const fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}
