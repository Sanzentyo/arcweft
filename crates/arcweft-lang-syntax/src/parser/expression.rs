//! Private expression-family event classification over the shared cursor.

use super::document::ShadowDocumentParser;
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, first_significant, range_contains, token_text, trimmed_end,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    let Some(first) = parser.current() else {
        parser.start(SyntaxKind::MissingExpression, role);
        parser.finish();
        return;
    };
    if parser.cursor() >= end {
        parser.start(SyntaxKind::MissingExpression, role);
        parser.finish();
        return;
    }
    let kind = classify_expression(parser, parser.cursor(), end, first);
    parser.start(kind, role);
    bump_until(parser, end);
    parser.finish();
}

pub(super) fn expression_is_call(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let Some(first) = first_significant(parser, start, end) else {
        return false;
    };
    let Some(first_token) = parser.token_at(first) else {
        return false;
    };
    matches!(
        first_token.kind(),
        SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
    ) && (first + 1..end).any(|index| token_text(parser, index) == Some("("))
}

fn classify_expression(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    first: LexToken,
) -> SyntaxKind {
    if range_has_binary_operator(parser, start, end) {
        return SyntaxKind::BinaryExpression;
    }
    match parser.text_of(first) {
        "&" => SyntaxKind::BorrowExpression,
        "*" => SyntaxKind::DereferenceExpression,
        "!" | "-" => SyntaxKind::UnaryExpression,
        "(" => SyntaxKind::TupleExpression,
        "[" if range_contains(parser, start, end, ";") => SyntaxKind::ArrayRepeatExpression,
        "[" => SyntaxKind::BracketSequenceExpression,
        "{" => SyntaxKind::BlockExpression,
        "if" => SyntaxKind::IfExpression,
        "match" => SyntaxKind::MatchExpression,
        "thread" => SyntaxKind::ThreadExpression,
        "_" => SyntaxKind::PlaceholderExpression,
        "." => SyntaxKind::ShortVariantExpression,
        _ if first.kind() == SyntaxKind::EntityReferenceToken => {
            SyntaxKind::EntityReferenceExpression
        }
        _ if first.kind() == SyntaxKind::LifetimeToken => SyntaxKind::LifetimePathExpression,
        _ if matches!(
            first.kind(),
            SyntaxKind::NumberToken
                | SyntaxKind::StringToken
                | SyntaxKind::RawStringToken
                | SyntaxKind::CharacterToken
        ) || matches!(parser.text_of(first), "true" | "false") =>
        {
            SyntaxKind::LiteralExpression
        }
        _ if expression_is_call(parser, start, end) => SyntaxKind::CallExpression,
        _ if range_contains(parser, start, end, "..")
            || range_contains(parser, start, end, "..=") =>
        {
            SyntaxKind::RangeExpression
        }
        _ => SyntaxKind::PathExpression,
    }
}

fn range_has_binary_operator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(text) = token_text(parser, index) else {
            break;
        };
        if depth == 0
            && matches!(
                text,
                "+" | "-"
                    | "*"
                    | "/"
                    | "%"
                    | "=="
                    | "!="
                    | "<"
                    | ">"
                    | "<="
                    | ">="
                    | "&&"
                    | "||"
                    | "|>"
            )
            && index > start
        {
            return true;
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}
