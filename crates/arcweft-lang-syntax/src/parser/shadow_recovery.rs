//! Shared lossless cursor scans, delimiters, and missing-token recovery.

use arcweft_source::SourceRange;

use super::document::ShadowDocumentParser;
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
    spelling: &str,
    diagnostic: &'static str,
) {
    parser.start(kind, SyntaxRole::CloseDelimiter);
    if parser.at(spelling) {
        parser.bump();
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
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

pub(super) fn expected(kind: SyntaxKind) -> ExpectedToken {
    ExpectedToken::try_new(kind).expect("real grammar token kind")
}

pub(super) fn find_header_boundary(parser: &ShadowDocumentParser<'_, '_>, start: usize) -> usize {
    find_top_level_boundary(parser, start, &["where", "requires", "ensures", "=", "{"])
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
    let (open, close) = match opening {
        "(" => ("(", ")"),
        "[" => ("[", "]"),
        "{" => ("{", "}"),
        "<" => ("<", ">"),
        _ => return None,
    };
    let mut depth = 0_usize;
    let mut index = start;
    while let Some(token) = parser.token_at(index) {
        match parser.text_of(token) {
            text if text == open => depth += 1,
            text if text == close && depth == 0 => return Some(index),
            text if text == close => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }
    None
}

pub(super) fn find_statement_terminator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<(usize, bool)> {
    let mut depth = 0_usize;
    for index in start..end {
        let token = parser.token_at(index)?;
        let text = parser.text_of(token);
        if depth == 0 && (text == ";" || token.kind() == SyntaxKind::NewlineToken) {
            return Some((index, text == ";"));
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

pub(super) fn range_contains(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    spelling: &str,
) -> bool {
    (start..end).any(|index| token_text(parser, index) == Some(spelling))
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
