//! Parenthesized and closure-expression events over the shared cursor.

use super::{CompletedNode, control, emit_expression};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::parser::document::ShadowDocumentParser;
use crate::parser::pattern::emit_pattern;
use crate::parser::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_matching_close,
    find_top_level_boundary, first_significant, trimmed_end,
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
