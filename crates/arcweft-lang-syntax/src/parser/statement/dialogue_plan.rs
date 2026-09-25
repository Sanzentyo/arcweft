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
    indented_item_end, indented_suite_interval, physical_line_end, trailing_braced_body_interval,
    trailing_owner_body_token,
};
use super::trigger::emit_trigger_pattern;
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

/// Finds the exact same-indent `with` that follows an indentation-owned
/// colon Dialogue body. The content suite's dedent is the ownership boundary.
pub(in crate::parser) fn colon_dialogue_plan_start(
    parser: &DocumentParser<'_, '_>,
    owner_start: usize,
    colon: usize,
    end: usize,
) -> Option<usize> {
    let content = indented_suite_interval(parser, owner_start, colon, end);
    let with = match content.issue() {
        None => first_significant(parser, content.end(), end)?,
        Some(super::indentation::IndentedSuiteIssue::MissingNewline) => {
            let line_end = physical_line_end(parser, owner_start, end);
            first_significant(parser, line_end.saturating_add(1), end)?
        }
        Some(super::indentation::IndentedSuiteIssue::MissingIndentedItem) => return None,
    };
    (token_text(parser, with) == Some("with")
        && super::indentation::token_indent(parser, with)
            == super::indentation::token_indent(parser, owner_start))
    .then_some(with)
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
        let terminator = line_plan_init_item_end(parser, start, close)
            .or_else(|| super::line_plan_defer_item_end(parser, start, close))
            .or_else(|| super::line_plan_on_item_end(parser, start, close))
            .map(|end| (end, false))
            .or_else(|| find_statement_terminator(parser, start, close));
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
        let item_end = line_plan_init_item_end(parser, start, interval.end()).unwrap_or(item_end);
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
    if parser.at("init") {
        emit_init(parser, end, item_kind, ordinal);
        return;
    }
    if parser.at("cancel")
        && first_significant(parser, parser.cursor().saturating_add(1), end)
            .and_then(|index| token_text(parser, index))
            == Some("on")
    {
        emit_cancel_rule(parser, end, item_kind, ordinal);
        return;
    }
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
                emit_callback_expression(
                    parser,
                    colon,
                    interval.first_item(),
                    interval.end(),
                    ordinal,
                );
                return;
            }
        }
        if let Some(colon) = head_body_introducer(parser, parser.cursor(), head_end)
            .filter(|index| token_text(parser, *index) == Some(":"))
        {
            emit_callback_expression(parser, colon, colon.saturating_add(1), end, ordinal);
            return;
        }
    }
    emit_statement_with_role(
        parser,
        end,
        item_kind,
        SyntaxRole::DialogueLinePlanItem(ordinal),
    );
}

fn emit_init(parser: &mut DocumentParser<'_, '_>, end: usize, item_kind: SyntaxKind, ordinal: u32) {
    let owner_start = parser.cursor();
    let head_end = physical_line_end(parser, owner_start, end);
    let Some(introducer) = head_body_introducer(parser, owner_start, head_end) else {
        emit_invalid_init(
            parser,
            end,
            ordinal,
            "syntax.dialogue.line_plan_init_missing_body",
        );
        return;
    };
    match token_text(parser, introducer) {
        Some("{") => {
            parser.start(
                SyntaxKind::DialogueLinePlanInit,
                SyntaxRole::DialogueLinePlanItem(ordinal),
            );
            parser.bump();
            bump_trivia_before(parser, end);
            bump_until(parser, introducer);
            let _ = super::emit_braced_statement_block_until(
                parser,
                end,
                item_kind,
                SyntaxKind::Block,
                SyntaxRole::Body,
                "syntax.dialogue.line_plan_init_missing_close",
            );
            parser.finish();
        }
        Some(":") => {
            let interval = indented_suite_interval(parser, owner_start, introducer, end);
            if interval.issue().is_some() {
                emit_invalid_init(
                    parser,
                    interval.end(),
                    ordinal,
                    "syntax.dialogue.line_plan_init_invalid_indent",
                );
                return;
            }
            parser.start(
                SyntaxKind::DialogueLinePlanInit,
                SyntaxRole::DialogueLinePlanItem(ordinal),
            );
            parser.bump();
            bump_trivia_before(parser, introducer);
            parser.start(SyntaxKind::ColonNode, SyntaxRole::Colon);
            bump_until(parser, introducer);
            parser.bump();
            parser.finish();
            bump_until(parser, interval.first_item());
            super::emit_unbraced_statement_block_until(
                parser,
                interval.end(),
                item_kind,
                SyntaxRole::Body,
            );
            bump_until(parser, interval.end());
            parser.finish();
        }
        _ => emit_invalid_init(
            parser,
            end,
            ordinal,
            "syntax.dialogue.line_plan_init_missing_body",
        ),
    }
}

fn emit_invalid_init(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
    code: &'static str,
) {
    let start = parser.current_offset();
    parser.start(
        SyntaxKind::ErrorStatement,
        SyntaxRole::DialogueLinePlanItem(ordinal),
    );
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(start, parser.current_offset()),
        if code == "syntax.dialogue.line_plan_init_invalid_indent" {
            "Dialogue Init requires an indented statement body after `:`"
        } else {
            "Dialogue Init requires a braced or indented statement body"
        },
    )));
}

/// Finds the full extent of a direct `init` item, including an indentation
/// suite nested inside a braced line plan.
fn line_plan_init_item_end(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    limit: usize,
) -> Option<usize> {
    if token_text(parser, start) != Some("init") {
        return None;
    }
    let head_end = physical_line_end(parser, start, limit);
    let introducer = head_body_introducer(parser, start, head_end)?;
    match token_text(parser, introducer)? {
        "{" => Some(
            find_matching_close_before(parser, introducer + 1, limit, "{")
                .map_or(limit, |close| close.saturating_add(1)),
        ),
        ":" => Some(indented_suite_interval(parser, start, introducer, limit).end()),
        _ => None,
    }
}

fn emit_cancel_rule(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    let owner_start = parser.cursor();
    parser.start(
        SyntaxKind::DialogueCancelRuleStatement,
        SyntaxRole::DialogueLinePlanItem(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    if parser.at("on") {
        parser.bump();
        bump_trivia_before(parser, end);
    } else {
        emit_cancel_rule_recovery(
            parser,
            end,
            SyntaxRole::Recovery(0),
            "syntax.dialogue.line_plan_cancel_missing_on",
            "Dialogue cancellation requires `on`",
        );
    }

    let body = trailing_braced_body_interval(parser, parser.cursor(), end);
    let head_end = physical_line_end(parser, parser.cursor(), end);
    let colon =
        super::indentation::trailing_owner_body_token(parser, parser.cursor(), head_end, true)
            .filter(|index| token_text(parser, *index) == Some(":"));
    let result_arrow = top_level_operator(parser, parser.cursor(), end, "=>");
    let trigger_end = body
        .map(|(open, _)| open)
        .or(colon)
        .or(result_arrow)
        .unwrap_or(end);
    emit_trigger_pattern(parser, trigger_end, SyntaxRole::Condition);
    bump_until(parser, trigger_end);
    if let Some((_, body_end)) = body {
        parser.start(SyntaxKind::DialogueCancelRuleBody, SyntaxRole::Body);
        let _ = super::emit_braced_thread_flow_block_until(
            parser,
            body_end,
            item_kind,
            SyntaxKind::Block,
            SyntaxRole::Element(0),
            "syntax.dialogue.line_plan_cancel_missing_close",
        );
        parser.finish();
        if first_significant(parser, parser.cursor(), end).is_some() {
            emit_cancel_rule_recovery(
                parser,
                end,
                SyntaxRole::TrailingRecovery(0),
                "syntax.dialogue.line_plan_cancel_trailing_tokens",
                "unexpected tokens after Dialogue cancellation body",
            );
            bump_until(parser, end);
        }
    } else if let Some(colon) = colon {
        emit_indented_cancel_rule_body(parser, colon, end, owner_start, item_kind);
    } else if result_arrow.is_some() {
        let at = parser.current_offset();
        parser.start(SyntaxKind::DialogueCancelRuleBody, SyntaxRole::Body);
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Recovery(0));
        parser.finish();
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.dialogue.line_plan_cancel_result_requires_content_owner",
            SourceRange::new(at, at),
            "value-returning cancellation with `=>` belongs to a ContentCall result owner; a line-plan cancel rule requires a statement body",
        )));
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::TrailingRecovery(0));
        bump_until(parser, end);
        parser.finish();
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::DialogueCancelRuleBody, SyntaxRole::Body);
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Recovery(0));
        parser.finish();
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.dialogue.line_plan_cancel_missing_body",
            SourceRange::new(at, at),
            "Dialogue cancellation requires a braced or indented statement body",
        )));
        bump_until(parser, end);
    }
    parser.finish();
}

fn emit_indented_cancel_rule_body(
    parser: &mut DocumentParser<'_, '_>,
    colon: usize,
    end: usize,
    owner_start: usize,
    item_kind: SyntaxKind,
) {
    let interval = indented_suite_interval(parser, owner_start, colon, end);
    parser.start(SyntaxKind::DialogueCancelRuleBody, SyntaxRole::Body);
    parser.start(SyntaxKind::ColonNode, SyntaxRole::Colon);
    bump_until(parser, colon);
    parser.bump();
    parser.finish();
    if interval.issue().is_some() {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Recovery(0));
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.dialogue.line_plan_cancel_invalid_indent",
            SourceRange::new(at, at),
            "Dialogue cancellation requires an indented statement body after `:`",
        )));
        bump_until(parser, interval.end());
        parser.finish();
        return;
    }

    parser.start(SyntaxKind::IndentedSuite, SyntaxRole::Element(0));
    bump_until(parser, interval.payload_start());
    bump_until(parser, interval.first_item());
    let suite_indent = interval
        .item_indent()
        .expect("accepted cancel rule body has an item indentation");
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
            super::emit_thread_flow_item(parser, significant_end, item_kind, ordinal);
        } else {
            parser.start(
                SyntaxKind::ErrorStatement,
                SyntaxRole::ThreadFlowItem(ordinal),
            );
            bump_until(parser, significant_end);
            parser.finish();
        }
        bump_until(parser, item_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget bounds cancellation-body item ordinals");
    }
    bump_until(parser, interval.end());
    parser.finish();
    parser.finish();
}

fn emit_cancel_rule_recovery(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    code: &'static str,
    message: &'static str,
) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, role);
    if matches!(role, SyntaxRole::TrailingRecovery(_)) {
        bump_until(parser, end);
    }
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(start, parser.current_offset()),
        message,
    )));
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
    body_start: usize,
    body_end: usize,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ExpressionStatement,
        SyntaxRole::DialogueLinePlanItem(ordinal),
    );
    emit_indented_callback_call(parser, colon, body_start, body_end, SyntaxRole::Initializer);
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
    let with = if let Some(with) = with {
        Some(with)
    } else if let Some(colon) = head_body_introducer(parser, statement_start, head_end)
        .filter(|index| token_text(parser, *index) == Some(":"))
    {
        colon_dialogue_plan_start(parser, statement_start, colon, limit)
    } else {
        if !saw_postfix_close {
            return None;
        }
        let next = first_significant(parser, head_end.saturating_add(1), limit)?;
        (token_text(parser, next) == Some("with")
            && super::indentation::token_indent(parser, next)
                == super::indentation::token_indent(parser, statement_start))
        .then_some(next)
    }?;
    let introducer = first_significant(parser, with.saturating_add(1), limit)?;
    match token_text(parser, introducer) {
        Some("{") => find_matching_close_before(parser, introducer + 1, limit, "{")
            .map_or(Some(limit), |close| Some(close.saturating_add(1))),
        Some(":") => Some(indented_suite_interval(parser, with, introducer, limit).end()),
        _ => None,
    }
}
