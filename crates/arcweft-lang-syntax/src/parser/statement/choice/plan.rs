//! Typed lifecycle-plan grammar attached to one canonical Choice statement.

use arcweft_source::SourceRange;

use super::super::super::cursor::DocumentParser;
use super::super::super::expression::emit_expression;
use super::super::super::pattern::emit_pattern;
use super::super::super::shadow_recovery::{
    bump_until, emit_open_delimiter, emit_required_punctuation, find_matching_close_before,
    find_statement_terminator, first_significant, token_text, trimmed_end,
};
use super::super::indentation::{
    IndentedSuiteInterval, SuiteLineIndentCursor, bump_trivia_before, head_body_introducer,
    indented_item_end, indented_suite_interval, physical_line_end, trailing_braced_body_interval,
};
use super::super::trigger::emit_trigger_pattern;
use super::super::{emit_braced_thread_flow_block_until, top_level_operator};
use super::{
    emit_choice_colon, emit_indented_suite_issue, emit_missing_body, emit_missing_expression,
    emit_recovery,
};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_choice_plan(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    suite_owner_start: usize,
) {
    let owner_start = parser.cursor();
    parser.start(SyntaxKind::ChoicePlan, SyntaxRole::Plan);
    parser.bump();
    bump_trivia_before(parser, end);

    let head_end = physical_line_end(parser, owner_start, end);
    let introducer = head_body_introducer(parser, parser.cursor(), head_end);
    match introducer.and_then(|index| token_text(parser, index).map(|text| (index, text))) {
        Some((open, "{")) => {
            if parser.cursor() < open {
                emit_recovery(
                    parser,
                    open,
                    SyntaxRole::Recovery(0),
                    "syntax.choice.plan_invalid_header",
                    "Choice lifecycle plan accepts only `with { ... }` or `with:`",
                );
            }
            bump_until(parser, open);
            emit_choice_plan_body(parser, end, item_kind);
        }
        Some((colon, ":")) => {
            if parser.cursor() < colon {
                emit_recovery(
                    parser,
                    colon,
                    SyntaxRole::Recovery(0),
                    "syntax.choice.plan_invalid_header",
                    "Choice lifecycle plan accepts only `with { ... }` or `with:`",
                );
            }
            bump_until(parser, colon);
            let interval = indented_suite_interval(parser, suite_owner_start, colon, end);
            emit_indented_choice_plan_body(parser, interval, item_kind);
        }
        _ => {
            if parser.cursor() < end {
                emit_recovery(
                    parser,
                    end,
                    SyntaxRole::Recovery(0),
                    "syntax.choice.plan_invalid_header",
                    "Choice lifecycle plan requires a braced or indented body",
                );
            }
            emit_missing_body(
                parser,
                SyntaxRole::Body,
                "syntax.choice.plan_missing_body",
                "missing Choice lifecycle-plan body",
            );
        }
    }

    if let Some(trailing) = first_significant(parser, parser.cursor(), end) {
        bump_until(parser, trailing);
        emit_recovery(
            parser,
            end,
            SyntaxRole::TrailingRecovery(0),
            "syntax.choice.plan_trailing_tokens",
            "unexpected tokens after Choice lifecycle-plan body",
        );
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_plan_body(parser: &mut DocumentParser<'_, '_>, end: usize, item_kind: SyntaxKind) {
    parser.start(SyntaxKind::ChoicePlanBody, SyntaxRole::Body);
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
        if start == significant_end {
            bump_until(parser, segment_end);
            continue;
        }
        emit_choice_plan_item(parser, significant_end, item_kind, ordinal);
        let consumed_end = if terminator.is_some_and(|(_, semicolon)| semicolon) {
            segment_end.saturating_add(1)
        } else {
            segment_end
        };
        bump_until(parser, consumed_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget keeps Choice plan ordinals within u32");
    }

    finish_plan_body(parser, close);
}

fn emit_indented_choice_plan_body(
    parser: &mut DocumentParser<'_, '_>,
    interval: IndentedSuiteInterval,
    item_kind: SyntaxKind,
) {
    parser.start(SyntaxKind::ChoicePlanBody, SyntaxRole::Body);
    emit_choice_colon(parser);
    parser.start(SyntaxKind::IndentedSuite, SyntaxRole::Element(0));
    bump_until(parser, interval.payload_start());

    if let Some(issue) = interval.issue() {
        emit_indented_suite_issue(parser, interval.end(), issue);
        bump_until(parser, interval.end());
        parser.finish();
        parser.finish();
        return;
    }

    bump_until(parser, interval.first_item());
    let suite_indent = interval
        .item_indent()
        .expect("accepted indented Choice plan has an item indent");
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
            is_choice_plan_item_head,
            |_, _| false,
        );
        let significant_end = trimmed_end(parser, start, item_end);
        if indent_cursor.observe(parser, start) == suite_indent {
            emit_choice_plan_item(parser, significant_end, item_kind, ordinal);
        } else {
            emit_recovery(
                parser,
                significant_end,
                SyntaxRole::ChoicePlanItem(ordinal),
                "syntax.choice.plan_invalid_item_indent",
                "Choice lifecycle-plan item indentation must match the first item",
            );
        }
        let consumed_end = if token_text(parser, item_end) == Some(";") {
            item_end.saturating_add(1).min(interval.end())
        } else {
            item_end
        };
        bump_until(parser, consumed_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget keeps Choice plan ordinals within u32");
    }
    bump_until(parser, interval.end());
    parser.finish();
    parser.finish();
}

fn is_choice_plan_item_head(kind: Option<SyntaxKind>, spelling: Option<&str>) -> bool {
    kind == Some(SyntaxKind::IdentifierToken)
        || spelling.is_some_and(|spelling| matches!(spelling, "timeout" | "cancel" | "on"))
}

fn emit_choice_plan_item(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    match parser.current_text() {
        Some("timeout") => emit_timeout(parser, end, item_kind, ordinal),
        Some("cancel") => emit_cancel(parser, end, item_kind, ordinal),
        Some("on") => emit_on_select(parser, end, item_kind, ordinal),
        _ if parser.current_kind() == Some(SyntaxKind::IdentifierToken) => {
            emit_assignment(parser, end, item_kind, ordinal);
        }
        _ => emit_recovery(
            parser,
            end,
            SyntaxRole::ChoicePlanItem(ordinal),
            "syntax.choice.plan_invalid_item",
            "unknown Choice lifecycle-plan item",
        ),
    }
}

fn emit_assignment(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ChoicePlanAssignment,
        SyntaxRole::ChoicePlanItem(ordinal),
    );
    parser.start(SyntaxKind::NameReference, SyntaxRole::Key);
    parser.bump();
    parser.finish();
    bump_trivia_before(parser, end);

    let equals = top_level_operator(parser, parser.cursor(), end, "=");
    if let Some(equals) = equals
        && parser.cursor() < equals
    {
        emit_recovery(
            parser,
            equals,
            SyntaxRole::Recovery(0),
            "syntax.choice.plan_assignment_invalid_key",
            "Choice lifecycle-plan assignment keys are one identifier",
        );
    }
    emit_required_punctuation(
        parser,
        SyntaxKind::EqualsNode,
        SyntaxRole::Equals,
        "=",
        "syntax.choice.plan_assignment_missing_equals",
        "Choice lifecycle-plan assignment requires `=`",
    );
    bump_trivia_before(parser, end);
    if parser.cursor() < end {
        emit_expression(parser, end, SyntaxRole::Value);
    } else {
        emit_missing_expression(
            parser,
            SyntaxRole::Value,
            "syntax.choice.plan_assignment_missing_value",
            "Choice lifecycle-plan assignment requires a value",
        );
    }
    bump_until(parser, end);
    let _ = item_kind;
    parser.finish();
}

fn emit_timeout(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ChoicePlanTimeout,
        SyntaxRole::ChoicePlanItem(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    let body = trailing_body_interval(parser, parser.cursor(), end);
    emit_required_expression(
        parser,
        body.map_or(end, |(open, _)| open),
        SyntaxRole::Operand,
        "syntax.choice.plan_timeout_missing_duration",
        "Choice timeout requires a duration expression",
    );
    bump_until(parser, body.map_or(end, |(open, _)| open));
    emit_required_action_body(
        parser,
        body,
        item_kind,
        "syntax.choice.plan_timeout_missing_body",
        "Choice timeout requires an action body",
        "syntax.choice.plan_timeout_missing_block_close",
    );
    emit_action_trailing_recovery(parser, end);
    bump_until(parser, end);
    parser.finish();
}

fn emit_cancel(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ChoicePlanCancel,
        SyntaxRole::ChoicePlanItem(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    if parser.at("on") {
        parser.bump();
        bump_trivia_before(parser, end);
    } else {
        emit_zero_width_recovery(
            parser,
            "syntax.choice.plan_cancel_missing_on",
            "Choice cancellation requires `on`",
        );
    }

    let body = trailing_body_interval(parser, parser.cursor(), end);
    let head_end = body.map_or(end, |(open, _)| open);
    if parser.cursor() < head_end {
        emit_trigger_pattern(parser, head_end, SyntaxRole::Condition);
    } else {
        emit_missing_expression(
            parser,
            SyntaxRole::Condition,
            "syntax.choice.plan_cancel_missing_trigger",
            "Choice cancellation requires a trigger pattern",
        );
    }
    bump_until(parser, head_end);
    emit_required_action_body(
        parser,
        body,
        item_kind,
        "syntax.choice.plan_cancel_missing_body",
        "Choice cancellation requires an action body",
        "syntax.choice.plan_cancel_missing_block_close",
    );
    emit_action_trailing_recovery(parser, end);
    bump_until(parser, end);
    parser.finish();
}

fn emit_on_select(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ChoicePlanOnSelect,
        SyntaxRole::ChoicePlanItem(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    if parser.at("select") {
        parser.bump();
        bump_trivia_before(parser, end);
    } else {
        emit_zero_width_recovery(
            parser,
            "syntax.choice.plan_on_select_missing_select",
            "Choice selection handler requires `select`",
        );
    }

    let body = trailing_body_interval(parser, parser.cursor(), end);
    let head_end = body.map_or(end, |(open, _)| open);
    if parser.cursor() < head_end {
        emit_pattern(parser, head_end, SyntaxRole::Pattern);
    } else {
        parser.start(SyntaxKind::MissingPattern, SyntaxRole::Pattern);
        parser.finish();
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.choice.plan_on_select_missing_pattern",
            SourceRange::new(at, at),
            "Choice selection handler requires a pattern",
        )));
    }
    bump_until(parser, head_end);
    emit_required_action_body(
        parser,
        body,
        item_kind,
        "syntax.choice.plan_on_select_missing_body",
        "Choice selection handler requires an action body",
        "syntax.choice.plan_on_select_missing_block_close",
    );
    emit_action_trailing_recovery(parser, end);
    bump_until(parser, end);
    parser.finish();
}

fn emit_required_expression(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    missing_code: &'static str,
    missing_message: &'static str,
) {
    if parser.cursor() < end {
        emit_expression(parser, end, role);
    } else {
        emit_missing_expression(parser, role, missing_code, missing_message);
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_required_action_body(
    parser: &mut DocumentParser<'_, '_>,
    body: Option<(usize, usize)>,
    item_kind: SyntaxKind,
    missing_code: &'static str,
    missing_message: &'static str,
    missing_close_code: &'static str,
) {
    if let Some((_, body_end)) = body {
        let _ = emit_braced_thread_flow_block_until(
            parser,
            body_end,
            item_kind,
            SyntaxKind::Block,
            SyntaxRole::Body,
            missing_close_code,
        );
    } else {
        emit_missing_body(parser, SyntaxRole::Body, missing_code, missing_message);
    }
}

fn trailing_body_interval(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    trailing_braced_body_interval(parser, start, trimmed_end(parser, start, end))
}

fn emit_action_trailing_recovery(parser: &mut DocumentParser<'_, '_>, end: usize) {
    if let Some(trailing) = first_significant(parser, parser.cursor(), end) {
        bump_until(parser, trailing);
        emit_recovery(
            parser,
            end,
            SyntaxRole::TrailingRecovery(0),
            "syntax.choice.plan_action_trailing_tokens",
            "unexpected tokens after Choice lifecycle action body",
        );
    }
}

fn emit_zero_width_recovery(
    parser: &mut DocumentParser<'_, '_>,
    code: &'static str,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        message,
    )));
}

fn finish_plan_body(parser: &mut DocumentParser<'_, '_>, close: usize) {
    if parser.cursor() == close && parser.at("}") {
        super::super::super::shadow_recovery::emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.choice.plan_missing_block_close",
        );
    } else {
        super::super::super::shadow_recovery::emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.choice.plan_missing_block_close",
            SourceRange::new(at, at),
            "missing closing `}` for Choice lifecycle plan",
        )));
    }
    parser.finish();
}
