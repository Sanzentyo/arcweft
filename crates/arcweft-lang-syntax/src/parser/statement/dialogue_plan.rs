//! Typed Dialogue line-plan grammar attached to one content application.

use arcweft_source::SourceRange;

use super::super::cursor::DocumentParser;
use super::super::expression::emit_indented_callback_call;
use super::super::pattern::emit_pattern;
use super::super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_matching_close_before,
    find_statement_terminator, first_significant, token_text, trimmed_end,
};
use super::indentation::{
    IndentedSuiteInterval, SuiteLineIndentCursor, bump_trivia_before, head_body_introducer,
    indented_item_end, indented_suite_interval, physical_line_end, trailing_owner_body_token,
};
use super::{emit_statement_with_role, top_level_operator};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(in crate::parser) fn emit_dialogue_line_plan(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> SourceRange {
    let owner_start = parser.cursor();
    let start = parser
        .current()
        .expect("Dialogue line plan retains its `with` token")
        .range()
        .start();
    parser.start(SyntaxKind::DialogueLinePlan, SyntaxRole::Plan);
    parser.bump();
    bump_trivia_before(parser, end);
    let head_end = physical_line_end(parser, owner_start, end);
    match head_body_introducer(parser, parser.cursor(), head_end)
        .and_then(|index| token_text(parser, index).map(|text| (index, text)))
    {
        Some((open, "{")) => {
            bump_until(parser, open);
            emit_braced_body(parser, end, item_kind);
        }
        Some((colon, ":")) => {
            bump_until(parser, colon);
            let interval = indented_suite_interval(parser, owner_start, colon, end);
            emit_indented_body(parser, interval, item_kind);
        }
        _ => {
            parser.start(SyntaxKind::DialogueLinePlanBody, SyntaxRole::Body);
            parser.start(SyntaxKind::MissingBody, SyntaxRole::Recovery(0));
            parser.finish();
            parser.finish();
            let at = parser.current_offset();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.dialogue.line_plan_missing_body",
                SourceRange::new(at, at),
                "Dialogue line plan requires `with { ... }` or `with:`",
            )));
        }
    }
    bump_until(parser, end);
    parser.finish();
    SourceRange::new(start, parser.current_offset())
}

fn emit_braced_body(parser: &mut DocumentParser<'_, '_>, end: usize, item_kind: SyntaxKind) {
    parser.start(SyntaxKind::DialogueLinePlanBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        bump_trivia_before(parser, close);
        if parser.cursor() >= close {
            break;
        }
        let start = parser.cursor();
        let terminator = find_statement_terminator(parser, start, close);
        let segment_end = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, segment_end);
        if start < significant_end {
            emit_line_plan_item(parser, significant_end, item_kind, ordinal);
            ordinal = ordinal
                .checked_add(1)
                .expect("the grammar budget bounds line-plan item ordinals");
        }
        bump_until(
            parser,
            if terminator.is_some_and(|(_, semicolon)| semicolon) {
                segment_end.saturating_add(1).min(close)
            } else {
                segment_end
            },
        );
    }
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.dialogue.line_plan_missing_close",
    );
    parser.finish();
}

fn emit_indented_body(
    parser: &mut DocumentParser<'_, '_>,
    interval: IndentedSuiteInterval,
    item_kind: SyntaxKind,
) {
    parser.start(SyntaxKind::DialogueLinePlanBody, SyntaxRole::Body);
    parser.start(SyntaxKind::ColonNode, SyntaxRole::Colon);
    parser.bump();
    parser.finish();
    parser.start(SyntaxKind::IndentedSuite, SyntaxRole::Element(0));
    bump_until(parser, interval.payload_start());
    if interval.issue().is_some() {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Recovery(0));
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.dialogue.line_plan_invalid_indent",
            SourceRange::new(at, at),
            "Dialogue line plan requires an indented body",
        )));
        bump_until(parser, interval.end());
        parser.finish();
        parser.finish();
        return;
    }
    bump_until(parser, interval.first_item());
    let suite_indent = interval
        .item_indent()
        .expect("accepted Dialogue line plan has an item indentation");
    let mut indent_cursor = SuiteLineIndentCursor::new(interval.first_item(), suite_indent);
    let mut ordinal = 0_u32;
    while parser.cursor() < interval.end() {
        bump_trivia_before(parser, interval.end());
        if parser.cursor() >= interval.end() {
            break;
        }
        let start = parser.cursor();
        let item_end = indented_item_end(
            parser,
            start,
            interval.end(),
            suite_indent,
            |_, _| true,
            |_, _| false,
        );
        let significant_end = trimmed_end(parser, start, item_end);
        if indent_cursor.observe(parser, start) == suite_indent {
            emit_line_plan_item(parser, significant_end, item_kind, ordinal);
        } else {
            parser.start(
                SyntaxKind::ErrorStatement,
                SyntaxRole::DialogueLinePlanItem(ordinal),
            );
            bump_until(parser, significant_end);
            parser.finish();
        }
        bump_until(parser, item_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget bounds line-plan item ordinals");
    }
    bump_until(parser, interval.end());
    parser.finish();
    parser.finish();
}

fn emit_line_plan_item(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    if parser.at("let")
        && let Some(equals) = top_level_operator(parser, parser.cursor(), end, "=")
    {
        let head_end = physical_line_end(parser, parser.cursor(), end);
        if let Some(colon) =
            trailing_owner_body_token(parser, equals.saturating_add(1), head_end, true)
            && token_text(parser, colon) == Some(":")
        {
            let interval = indented_suite_interval(parser, parser.cursor(), colon, end);
            if interval.issue().is_none() {
                emit_callback_let(parser, equals, colon, interval, ordinal);
                return;
            }
        }
    }
    if parser.at("at") {
        let head_end = physical_line_end(parser, parser.cursor(), end);
        if let Some(colon) = trailing_owner_body_token(parser, parser.cursor(), head_end, true)
            && token_text(parser, colon) == Some(":")
        {
            let interval = indented_suite_interval(parser, parser.cursor(), colon, end);
            if interval.issue().is_none() {
                emit_callback_expression(parser, colon, interval, ordinal);
                return;
            }
        }
    }
    emit_statement_with_role(
        parser,
        end,
        item_kind,
        SyntaxRole::DialogueLinePlanItem(ordinal),
    );
}

fn emit_callback_let(
    parser: &mut DocumentParser<'_, '_>,
    equals: usize,
    colon: usize,
    interval: IndentedSuiteInterval,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::LetStatement,
        SyntaxRole::DialogueLinePlanItem(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, equals);
    emit_pattern(parser, equals, SyntaxRole::Pattern);
    bump_until(parser, equals);
    parser.bump();
    bump_trivia_before(parser, colon);
    emit_indented_callback_call(
        parser,
        colon,
        interval.first_item(),
        interval.end(),
        SyntaxRole::Initializer,
    );
    bump_until(parser, interval.end());
    parser.finish();
}

fn emit_callback_expression(
    parser: &mut DocumentParser<'_, '_>,
    colon: usize,
    interval: IndentedSuiteInterval,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ExpressionStatement,
        SyntaxRole::DialogueLinePlanItem(ordinal),
    );
    emit_indented_callback_call(
        parser,
        colon,
        interval.first_item(),
        interval.end(),
        SyntaxRole::Initializer,
    );
    parser.finish();
}

/// Finds an exact eligible `with` continuation and returns the exclusive end
/// of its body. The caller has already selected a statement-owned expression
/// interval, so only token geometry participates here.
pub(super) fn dialogue_plan_end(
    parser: &DocumentParser<'_, '_>,
    statement_start: usize,
    limit: usize,
) -> Option<usize> {
    let head_end = physical_line_end(parser, statement_start, limit);
    let mut depth = 0_usize;
    let mut saw_postfix_close = false;
    let mut with = None;
    for index in statement_start..head_end {
        let text = token_text(parser, index)?;
        if depth == 0 && text == "with" {
            with = Some(index);
            break;
        }
        match text {
            "(" | "[" | "{" => depth = depth.saturating_add(1),
            ")" | "}" => depth = depth.saturating_sub(1),
            "]" => {
                depth = depth.saturating_sub(1);
                saw_postfix_close |= depth == 0;
            }
            _ => {}
        }
    }
    if !saw_postfix_close {
        return None;
    }
    let with = with.or_else(|| {
        let next = first_significant(parser, head_end.saturating_add(1), limit)?;
        (token_text(parser, next) == Some("with")
            && super::indentation::token_indent(parser, next)
                == super::indentation::token_indent(parser, statement_start))
        .then_some(next)
    })?;
    let introducer = first_significant(parser, with.saturating_add(1), limit)?;
    match token_text(parser, introducer) {
        Some("{") => find_matching_close_before(parser, introducer + 1, limit, "{")
            .map_or(Some(limit), |close| Some(close.saturating_add(1))),
        Some(":") => Some(indented_suite_interval(parser, with, introducer, limit).end()),
        _ => None,
    }
}
