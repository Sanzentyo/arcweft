//! Parenthesized and closure-expression events over the shared cursor.

use super::{CompletedNode, control, emit_expression};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::parser::document::ShadowDocumentParser;
use crate::parser::pattern::emit_pattern;
use crate::parser::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_matching_close,
    find_top_level_boundary, first_significant, range_contains, token_text, trimmed_end,
};
use crate::parser::type_ref::emit_type;

pub(super) fn emit_parenthesized(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let close = find_matching_close(parser, parser.cursor() + 1, "(")
        .unwrap_or(end)
        .min(end);
    let first = first_significant(parser, parser.cursor() + 1, close);
    let comma = find_top_level_boundary(parser, parser.cursor() + 1, &[",", ")"]);
    if first.is_some() && comma >= close {
        return emit_delimited_group(parser, close, role);
    }
    emit_tuple(parser, close, role)
}

fn emit_delimited_group(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::DelimitedGroup, role);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.bump_trivia();
    emit_expression(parser, close, SyntaxRole::Operand);
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.expression.missing_parenthesis_close",
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_tuple(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::TupleExpression, role);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at(")") {
            break;
        }
        let element_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
        emit_expression(parser, element_end, SyntaxRole::Element(ordinal));
        bump_until(parser, element_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.expression.missing_parenthesis_close",
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_bracket_sequence(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let close = find_matching_close(parser, parser.cursor() + 1, "[")
        .unwrap_or(end)
        .min(end);
    let content_start = parser.cursor() + 1;
    let kind = if range_contains(parser, content_start, close, ";") {
        SyntaxKind::ArrayRepeatExpression
    } else if is_compact_integer_sequence(parser, content_start, close) {
        SyntaxKind::NumericBracketSequenceExpression
    } else {
        SyntaxKind::BracketSequenceExpression
    };
    parser.start(kind, role);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    if kind == SyntaxKind::NumericBracketSequenceExpression {
        bump_until(parser, close);
    } else {
        emit_bracket_elements(parser, close);
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.expression.missing_bracket_close",
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_bracket_elements(parser: &mut ShadowDocumentParser<'_, '_>, close: usize) {
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("]") {
            break;
        }
        let element_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", ";", "]"]).min(close);
        emit_expression(parser, element_end, SyntaxRole::Element(ordinal));
        bump_until(parser, element_end);
        ordinal = ordinal.saturating_add(1);
        if matches!(parser.current_text(), Some("," | ";")) {
            parser.bump();
        } else {
            break;
        }
    }
}

fn is_compact_integer_sequence(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let mut common_suffix = None;
    let mut saw_literal = false;
    let mut expect_literal = true;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return false;
        };
        if matches!(
            token.kind(),
            SyntaxKind::WhitespaceToken | SyntaxKind::NewlineToken | SyntaxKind::CommentToken
        ) {
            continue;
        }
        let text = parser.text_of(token);
        if expect_literal {
            if token.kind() != SyntaxKind::NumberToken {
                return false;
            }
            let Some(suffix) = integer_suffix(text) else {
                return false;
            };
            if saw_literal && common_suffix != Some(suffix) {
                return false;
            }
            common_suffix = Some(suffix);
            saw_literal = true;
            expect_literal = false;
        } else if text == "," {
            expect_literal = true;
        } else {
            return false;
        }
    }
    saw_literal
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum IntegerSuffix {
    None,
    Explicit(&'static str),
}

fn integer_suffix(source: &str) -> Option<IntegerSuffix> {
    #[derive(Clone, Copy)]
    enum Digits {
        Binary,
        Octal,
        Decimal,
        Hexadecimal,
    }

    const SUFFIXES: [&str; 12] = [
        "isize", "usize", "i128", "u128", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8",
    ];
    let suffix = SUFFIXES.into_iter().find(|suffix| source.ends_with(suffix));
    let digits = suffix.map_or(source, |suffix| {
        source
            .strip_suffix(suffix)
            .unwrap_or(source)
            .trim_end_matches('_')
    });
    let (body, digit_kind) = if let Some(body) = digits.strip_prefix("0x") {
        (body, Digits::Hexadecimal)
    } else if let Some(body) = digits.strip_prefix("0X") {
        (body, Digits::Hexadecimal)
    } else if let Some(body) = digits.strip_prefix("0b") {
        (body, Digits::Binary)
    } else if let Some(body) = digits.strip_prefix("0B") {
        (body, Digits::Binary)
    } else if let Some(body) = digits.strip_prefix("0o") {
        (body, Digits::Octal)
    } else if let Some(body) = digits.strip_prefix("0O") {
        (body, Digits::Octal)
    } else {
        (digits, Digits::Decimal)
    };
    let mut meaningful = body.chars().filter(|ch| *ch != '_').peekable();
    meaningful.peek()?;
    meaningful
        .all(|ch| match digit_kind {
            Digits::Binary => matches!(ch, '0' | '1'),
            Digits::Octal => matches!(ch, '0'..='7'),
            Digits::Decimal => ch.is_ascii_digit(),
            Digits::Hexadecimal => ch.is_ascii_hexdigit(),
        })
        .then_some(suffix.map_or(IntegerSuffix::None, IntegerSuffix::Explicit))
}

pub(super) fn emit_closure(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::ClosureExpression, role);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    if parser.at("||") {
        parser.bump();
    } else {
        parser.bump();
        emit_closure_parameters(parser, end);
        if parser.at("|") {
            parser.bump();
        }
    }
    parser.finish();
    parser.bump_trivia();

    if parser.at("->") {
        emit_closure_return_type(parser, end);
        parser.bump_trivia();
    }
    if parser.at("{") {
        control::emit_block_expression(parser, end, SyntaxRole::Body);
    } else {
        emit_expression(parser, end, SyntaxRole::Body);
    }
    parser.finish();
    CompletedNode { start_event }
}

fn emit_closure_parameters(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let close = find_top_level_boundary(parser, parser.cursor(), &["|"]).min(end);
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("|") {
            break;
        }
        let parameter_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", "|"]).min(close);
        let type_separator =
            find_top_level_boundary(parser, parser.cursor(), &[":", ",", "|"]).min(parameter_end);
        parser.start(SyntaxKind::ClosureParameter, SyntaxRole::Parameter(ordinal));
        emit_pattern(parser, type_separator, SyntaxRole::ParameterPattern);
        bump_until(parser, type_separator);
        if parser.at(":") {
            parser.bump();
            parser.bump_trivia();
            emit_type(parser, parameter_end, SyntaxRole::ParameterType);
            bump_until(parser, parameter_end);
        }
        parser.finish();
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    bump_until(parser, close);
}

fn emit_closure_return_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.start(SyntaxKind::ReturnType, SyntaxRole::ReturnType);
    parser.bump();
    parser.bump_trivia();
    let body = find_top_level_boundary(parser, parser.cursor(), &["{"]).min(end);
    let type_end = trimmed_end(parser, parser.cursor(), body);
    emit_type(parser, type_end, SyntaxRole::Type);
    bump_until(parser, body);
    parser.finish();
}

pub(super) fn has_braced_body(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> bool {
    block_open(parser, end).is_some()
}

pub(super) fn is_nominal_record_head(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> bool {
    let Some(open) = block_open(parser, end) else {
        return false;
    };
    (parser.cursor()..open)
        .rev()
        .find_map(|index| {
            let token = parser.token_at(index)?;
            (!matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken | SyntaxKind::NewlineToken | SyntaxKind::CommentToken
            ))
            .then(|| parser.text_of(token))
        })
        .and_then(|name| name.chars().next())
        .is_some_and(char::is_uppercase)
}

pub(super) fn emit_braced_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let close = find_matching_close(parser, parser.cursor() + 1, "{")
        .unwrap_or(end)
        .min(end);
    if looks_like_record_literal(parser, parser.cursor() + 1, close) {
        emit_record_literal(parser, close, role)
    } else {
        control::emit_block_expression(parser, end, role)
    }
}

pub(super) fn emit_record_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let open = block_open(parser, end).unwrap_or(end);
    parser.start(SyntaxKind::RecordExpression, role);
    crate::parser::path::emit_path(parser, open, SyntaxRole::Target);
    bump_until(parser, open);
    emit_record_fields(parser, end);
    parser.finish();
    CompletedNode { start_event }
}

fn emit_record_literal(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::RecordLiteralExpression, role);
    emit_record_fields(parser, end);
    parser.finish();
    CompletedNode { start_event }
}

fn emit_record_fields(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    debug_assert!(parser.at("{"));
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("}") {
            break;
        }
        let field_end = record_field_boundary(parser, parser.cursor(), close);
        emit_record_field(parser, field_end, ordinal);
        bump_until(parser, field_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else if parser.cursor() >= close
            || parser.current_kind() != Some(SyntaxKind::NewlineToken)
        {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.expression.missing_record_close",
    );
}

fn record_field_boundary(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return index;
        };
        let text = parser.text_of(token);
        if depth == 0 && (text == "," || token.kind() == SyntaxKind::NewlineToken) {
            return index;
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    end
}

fn emit_record_field(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u16) {
    parser.start(SyntaxKind::RecordField, SyntaxRole::Field(ordinal));
    let separator = find_top_level_boundary(parser, parser.cursor(), &["=", ":"]).min(end);
    let name_end = if separator < end { separator } else { end };
    parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
    bump_until(parser, trimmed_end(parser, parser.cursor(), name_end));
    parser.finish();
    bump_until(parser, separator);
    if separator < end {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, end, SyntaxRole::Initializer);
        bump_until(parser, end);
    }
    parser.finish();
}

pub(super) fn emit_computation_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let open = block_open(parser, end).unwrap_or(end);
    parser.start(SyntaxKind::ComputationBlockExpression, role);
    bump_until(parser, open);
    if parser.at("{") {
        control::emit_block_contents(parser, SyntaxRole::Body);
    }
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_named_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let open = block_open(parser, end).unwrap_or(end);
    parser.start(SyntaxKind::NamedBlockExpression, role);
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < open {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        bump_until(parser, trimmed_end(parser, parser.cursor(), open));
        parser.finish();
        bump_until(parser, open);
    }
    if parser.at("{") {
        control::emit_block_contents(parser, SyntaxRole::Body);
    }
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_thread_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let open = block_open(parser, end).unwrap_or(end);
    parser.start(SyntaxKind::ThreadExpression, role);
    parser.bump();
    parser.bump_trivia();
    if parser.at("detached") {
        parser.bump();
        parser.bump_trivia();
    }
    if parser.cursor() < open {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        bump_until(parser, trimmed_end(parser, parser.cursor(), open));
        parser.finish();
        bump_until(parser, open);
    }
    if parser.at("{") {
        control::emit_block_contents(parser, SyntaxRole::Body);
    }
    parser.finish();
    CompletedNode { start_event }
}

fn block_open(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> Option<usize> {
    let open = find_top_level_boundary(parser, parser.cursor(), &["{"]).min(end);
    (open < end && token_text(parser, open) == Some("{")).then_some(open)
}

fn looks_like_record_literal(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let Some(first) = first_significant(parser, start, end) else {
        return false;
    };
    if token_text(parser, first).is_some_and(|head| {
        matches!(
            head,
            "let"
                | "return"
                | "out"
                | "goto"
                | "thread"
                | "defer"
                | "yield"
                | "signal"
                | "wait"
                | "on"
                | "if"
                | "loop"
                | "while"
                | "for"
                | "match"
                | "break"
                | "continue"
        )
    }) {
        return false;
    }
    let boundary = find_top_level_boundary(parser, first, &["=", ":", ",", ";"]);
    boundary < end && token_text(parser, boundary) != Some(";")
}
