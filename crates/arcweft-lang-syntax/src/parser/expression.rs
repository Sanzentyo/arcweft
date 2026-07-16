//! Private Pratt expression grammar over the shared document cursor.

use super::document::ShadowDocumentParser;
use super::path::emit_path;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_top_level_boundary, range_contains,
    trimmed_end,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletedNode {
    start_event: usize,
}

pub(super) fn emit_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    if parser.cursor() >= end {
        parser.start(SyntaxKind::MissingExpression, role);
        parser.finish();
        return;
    }

    let completed = parse_binding_power(parser, end, 0, role);
    if parser.cursor() < end {
        parser.insert_start(completed.start_event, SyntaxKind::ErrorExpression, role);
        parser.set_start_role(completed.start_event + 1, SyntaxRole::Operand);
        bump_until(parser, end);
        parser.finish();
    }
}

pub(super) fn expression_is_call(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let mut depth = 0_usize;
    let mut saw_callee = false;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        let text = parser.text_of(token);
        if token.kind() == SyntaxKind::WhitespaceToken || token.kind() == SyntaxKind::CommentToken {
            continue;
        }
        if !saw_callee {
            saw_callee = matches!(
                token.kind(),
                SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
            );
            if !saw_callee {
                return false;
            }
            continue;
        }
        match text {
            "(" if depth == 0 => return true,
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "." | "?." | "::" => {}
            _ if token.kind() == SyntaxKind::IdentifierToken => {}
            _ => return false,
        }
    }
    false
}

fn parse_binding_power(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    minimum: u8,
    role: SyntaxRole,
) -> CompletedNode {
    let mut left = parse_prefix(parser, end, role);

    while let Some((operator_index, _, operator)) = parser.next_significant() {
        if operator_index >= end {
            break;
        }

        if is_postfix_operator(operator) {
            bump_until(parser, operator_index);
            left = emit_postfix(parser, end, left, role, operator);
            continue;
        }

        let Some((left_power, right_power, kind)) = binary_binding_power(operator) else {
            break;
        };
        if left_power < minimum {
            break;
        }

        bump_until(parser, operator_index);
        parser.insert_start(left.start_event, kind, role);
        parser.set_start_role(left.start_event + 1, SyntaxRole::LeftOperand);
        parser.bump();
        parser.bump_trivia();
        if parser.cursor() < end {
            parse_binding_power(parser, end, right_power, SyntaxRole::RightOperand);
        } else {
            parser.start(SyntaxKind::MissingExpression, SyntaxRole::RightOperand);
            parser.finish();
        }
        parser.finish();
        left = CompletedNode {
            start_event: left.start_event,
        };
    }

    left
}

fn parse_prefix(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let Some(token) = parser.current() else {
        parser.start(SyntaxKind::MissingExpression, role);
        parser.finish();
        return CompletedNode { start_event };
    };
    let text = parser.text_of(token);

    match text {
        "&" => emit_prefix_operand(parser, end, SyntaxKind::BorrowExpression, role, true),
        "*" => emit_prefix_operand(parser, end, SyntaxKind::DereferenceExpression, role, false),
        "!" | "-" | "+" => {
            emit_prefix_operand(parser, end, SyntaxKind::UnaryExpression, role, false)
        }
        "await" => emit_prefix_operand(parser, end, SyntaxKind::AwaitExpression, role, false),
        "thread" => emit_prefix_operand(parser, end, SyntaxKind::ThreadExpression, role, false),
        "(" => emit_tuple(parser, end, role),
        "[" => emit_bracket_sequence(parser, end, role),
        "." => emit_short_variant(parser, end, role),
        "{" => emit_flat(parser, end, SyntaxKind::BlockExpression, role),
        "if" => emit_flat(parser, end, SyntaxKind::IfExpression, role),
        "match" => emit_flat(parser, end, SyntaxKind::MatchExpression, role),
        "|" => emit_flat(parser, end, SyntaxKind::ClosureExpression, role),
        "_" => emit_single(parser, SyntaxKind::PlaceholderExpression, role),
        "true" | "false" => emit_single(parser, SyntaxKind::LiteralExpression, role),
        _ if token.kind() == SyntaxKind::EntityReferenceToken => {
            emit_single(parser, SyntaxKind::EntityReferenceExpression, role)
        }
        _ if token.kind() == SyntaxKind::LifetimeToken => {
            emit_path_like(parser, end, SyntaxKind::LifetimePathExpression, role)
        }
        _ if is_literal(token.kind()) => emit_single(parser, SyntaxKind::LiteralExpression, role),
        _ if matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        ) =>
        {
            emit_path_like(parser, end, SyntaxKind::PathExpression, role)
        }
        _ => emit_single(parser, SyntaxKind::ErrorExpression, role),
    }
}

fn emit_prefix_operand(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
    accepts_mutability: bool,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(kind, role);
    parser.bump();
    parser.bump_trivia();
    if accepts_mutability && parser.at("mut") {
        parser.bump();
        parser.bump_trivia();
    }
    if parser.cursor() < end {
        parse_binding_power(parser, end, 90, SyntaxRole::Operand);
    } else {
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Operand);
        parser.finish();
    }
    parser.finish();
    CompletedNode { start_event }
}

fn emit_tuple(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::TupleExpression, role);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end || parser.at(")") {
            break;
        }
        let element_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(end);
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

fn emit_bracket_sequence(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let close = find_top_level_boundary(parser, parser.cursor() + 1, &["]"]).min(end);
    let kind = if range_contains(parser, parser.cursor() + 1, close, ";") {
        SyntaxKind::ArrayRepeatExpression
    } else {
        SyntaxKind::BracketSequenceExpression
    };
    parser.start(kind, role);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end || parser.at("]") {
            break;
        }
        let element_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", ";", "]"]).min(end);
        emit_expression(parser, element_end, SyntaxRole::Element(ordinal));
        bump_until(parser, element_end);
        ordinal = ordinal.saturating_add(1);
        if matches!(parser.current_text(), Some("," | ";")) {
            parser.bump();
        } else {
            break;
        }
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

fn emit_short_variant(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::ShortVariantExpression, role);
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Target);
        parser.bump();
        parser.finish();
    } else {
        parser.start(SyntaxKind::MissingName, SyntaxRole::Target);
        parser.finish();
    }
    parser.finish();
    CompletedNode { start_event }
}

fn emit_path_like(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    expression_kind: SyntaxKind,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(expression_kind, role);
    emit_path(parser, end, SyntaxRole::Target);
    parser.finish();
    CompletedNode { start_event }
}

fn emit_single(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(kind, role);
    parser.bump();
    parser.finish();
    CompletedNode { start_event }
}

fn emit_flat(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(kind, role);
    bump_until(parser, end);
    parser.finish();
    CompletedNode { start_event }
}

fn emit_postfix(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
    operator: &str,
) -> CompletedNode {
    match operator {
        "(" => emit_call(parser, end, left, role),
        "[" => emit_index(parser, end, left, role),
        "." | "?." => emit_select(parser, end, left, role),
        "?" => emit_try(parser, left, role),
        _ => left,
    }
}

fn emit_call(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    parser.insert_start(left.start_event, SyntaxKind::CallExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Callee);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ArgumentList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end || parser.at(")") {
            break;
        }
        let argument_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(end);
        parser.start(SyntaxKind::CallArgument, SyntaxRole::Argument(ordinal));
        emit_expression(parser, argument_end, SyntaxRole::Operand);
        bump_until(parser, argument_end);
        parser.finish();
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
        "syntax.expression.missing_call_close",
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn emit_index(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    parser.insert_start(left.start_event, SyntaxKind::IndexExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Target);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    parser.bump_trivia();
    let index_end = find_top_level_boundary(parser, parser.cursor(), &["]"]).min(end);
    emit_expression(parser, index_end, SyntaxRole::Argument(0));
    bump_until(parser, index_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.expression.missing_index_close",
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn emit_select(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    parser.insert_start(left.start_event, SyntaxKind::SelectExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Target);
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Field(0));
        parser.bump();
        parser.finish();
    } else {
        parser.start(SyntaxKind::MissingName, SyntaxRole::Field(0));
        parser.finish();
    }
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn emit_try(
    parser: &mut ShadowDocumentParser<'_, '_>,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    parser.insert_start(left.start_event, SyntaxKind::TryExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Operand);
    parser.bump();
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn is_postfix_operator(operator: &str) -> bool {
    matches!(operator, "(" | "[" | "." | "?." | "?")
}

fn binary_binding_power(operator: &str) -> Option<(u8, u8, SyntaxKind)> {
    let (power, kind) = match operator {
        "|>" => (1, SyntaxKind::PipeExpression),
        "||" | "??" => (3, SyntaxKind::BinaryExpression),
        "&&" => (5, SyntaxKind::BinaryExpression),
        "==" | "!=" => (7, SyntaxKind::BinaryExpression),
        "<" | "<=" | ">" | ">=" | "in" => (9, SyntaxKind::BinaryExpression),
        ".." | "..=" => (11, SyntaxKind::RangeExpression),
        "+" | "-" => (13, SyntaxKind::BinaryExpression),
        "*" | "/" | "%" => (15, SyntaxKind::BinaryExpression),
        _ => return None,
    };
    Some((power, power + 1, kind))
}

const fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NumberToken
            | SyntaxKind::StringToken
            | SyntaxKind::RawStringToken
            | SyntaxKind::CharacterToken
    )
}
