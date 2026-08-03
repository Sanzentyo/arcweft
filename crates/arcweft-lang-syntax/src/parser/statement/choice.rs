//! One-pass typed Choice grammar over the shared statement cursor.

mod option;
mod plan;

use arcweft_source::SourceRange;

use super::super::cursor::ShadowDocumentParser;
use super::super::expression::{CompletedNode, emit_entity_reference, emit_expression};
use super::super::pattern::emit_pattern;
use super::super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    find_matching_close_before, find_statement_terminator, first_significant, token_text,
    trimmed_end,
};
use super::indentation::{
    IndentedSuiteInterval, IndentedSuiteIssue, SuiteLineIndentCursor, bump_trivia_before,
    has_newline_between, head_body_introducer, indented_item_end, indented_suite_interval,
    physical_line_end, physical_line_owner_start, starts_physical_line, token_indent,
    trailing_braced_body_interval, trailing_owner_body_token,
};
use super::{
    emit_item_expression, emit_statement_with_role, find_match_arm_end, top_level_operator,
};
use crate::expressions::{ExpressionProjection, PendingExpressionProjection};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

use option::{emit_choice_option_field, is_choice_option_field_head};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChoiceBodyIntroducer {
    Braced(usize),
    Indented(IndentedSuiteInterval),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChoiceStatementExtent {
    body: Option<ChoiceBodyIntroducer>,
    body_end: usize,
    plan: Option<usize>,
    end: usize,
}

pub(super) fn logical_choice_end(
    parser: &ShadowDocumentParser<'_, '_>,
    choice_start: usize,
    suite_owner_start: usize,
    limit: usize,
) -> usize {
    choice_expression_extent(parser, choice_start, suite_owner_start, limit).end
}

fn choice_expression_extent(
    parser: &ShadowDocumentParser<'_, '_>,
    choice_start: usize,
    suite_owner_start: usize,
    limit: usize,
) -> ChoiceStatementExtent {
    let head_end = physical_line_end(parser, choice_start, limit);
    let with_on_head = top_level_operator(parser, choice_start.saturating_add(1), head_end, "with");
    let raw_body = head_body_introducer(parser, choice_start.saturating_add(1), head_end);
    let body_start = raw_body.filter(|body| with_on_head.is_none_or(|with| *body < with));
    let body = body_start.and_then(|index| match token_text(parser, index) {
        Some("{") => Some(ChoiceBodyIntroducer::Braced(index)),
        Some(":") => Some(ChoiceBodyIntroducer::Indented(indented_suite_interval(
            parser,
            suite_owner_start,
            index,
            limit,
        ))),
        _ => None,
    });
    let body_end = match body {
        Some(ChoiceBodyIntroducer::Braced(open)) => {
            find_matching_close_before(parser, open + 1, limit, "{")
                .map_or(limit, |close| close + 1)
        }
        Some(ChoiceBodyIntroducer::Indented(interval)) => interval.end(),
        None => with_on_head.unwrap_or(head_end),
    };

    let plan = with_on_head.filter(|with| *with >= body_end).or_else(|| {
        let candidate = first_significant(parser, body_end, limit)?;
        if token_text(parser, candidate) != Some("with") {
            return None;
        }
        match body {
            Some(ChoiceBodyIntroducer::Indented(_)) => {
                if token_indent(parser, candidate) != token_indent(parser, suite_owner_start) {
                    return None;
                }
            }
            Some(ChoiceBodyIntroducer::Braced(_)) => {
                let later_line = starts_physical_line(parser, body_end)
                    || has_newline_between(parser, body_end, candidate);
                if later_line
                    && token_indent(parser, candidate) != token_indent(parser, suite_owner_start)
                {
                    return None;
                }
            }
            None => {
                if token_indent(parser, candidate) != token_indent(parser, suite_owner_start) {
                    return None;
                }
            }
        }
        Some(candidate)
    });
    let end = plan.map_or_else(
        || {
            if matches!(body, Some(ChoiceBodyIntroducer::Indented(_))) {
                return body_end;
            }
            let trailing = first_significant(parser, body_end, limit);
            let same_line = !starts_physical_line(parser, body_end)
                && trailing.is_some_and(|token| !has_newline_between(parser, body_end, token));
            if same_line {
                find_statement_terminator(parser, body_end, limit).map_or_else(
                    || physical_line_end(parser, body_end, limit),
                    |(terminator, _)| terminator,
                )
            } else {
                body_end
            }
        },
        |with| plan_end(parser, suite_owner_start, with, limit),
    );
    ChoiceStatementExtent {
        body,
        body_end,
        plan,
        end,
    }
}

fn plan_end(
    parser: &ShadowDocumentParser<'_, '_>,
    choice_start: usize,
    with: usize,
    limit: usize,
) -> usize {
    let head_end = physical_line_end(parser, with, limit);
    match head_body_introducer(parser, with.saturating_add(1), head_end)
        .and_then(|index| token_text(parser, index).map(|text| (index, text)))
    {
        Some((_open, "{")) => {
            find_statement_terminator(parser, with, limit).map_or(limit, |(end, _)| end)
        }
        Some((colon, ":")) => indented_suite_interval(parser, choice_start, colon, limit).end(),
        _ => head_end,
    }
}

pub(in crate::parser) fn emit_choice_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::ChoiceExpression, role);
    let start = parser.cursor();
    let suite_owner_start = physical_line_owner_start(parser, start);
    let item_kind = SyntaxKind::FunctionItem;
    let extent = choice_expression_extent(parser, start, suite_owner_start, end);
    let header_end = match extent.body {
        Some(ChoiceBodyIntroducer::Braced(open)) => open,
        Some(ChoiceBodyIntroducer::Indented(interval)) => interval.colon(),
        None => extent.plan.unwrap_or(extent.end),
    };
    parser.bump();
    bump_trivia_before(parser, header_end);

    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        emit_entity_reference(parser, SyntaxRole::PublicId);
        bump_trivia_before(parser, header_end);
    }
    if parser.cursor() < header_end {
        emit_recovery(
            parser,
            header_end,
            SyntaxRole::Recovery(0),
            "syntax.choice.invalid_id",
            "Choice ID must be one static entity reference",
        );
    }
    bump_until(parser, header_end);

    match extent.body {
        Some(ChoiceBodyIntroducer::Braced(open)) => {
            bump_until(parser, open);
            emit_choice_body(parser, extent.body_end, item_kind, SyntaxRole::Body);
        }
        Some(ChoiceBodyIntroducer::Indented(interval)) => {
            bump_until(parser, interval.colon());
            emit_indented_choice_body(parser, interval, item_kind, SyntaxRole::Body);
        }
        None => {
            emit_missing_body(
                parser,
                SyntaxRole::Body,
                "syntax.choice.missing_body",
                "missing Choice body",
            );
        }
    }

    if let Some(with) = extent.plan {
        bump_until(parser, with);
        plan::emit_choice_plan(parser, extent.end, item_kind, start);
    }
    bump_until(parser, extent.end);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::Choice, Vec::new()),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_indented_choice_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    interval: IndentedSuiteInterval,
    item_kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(SyntaxKind::ChoiceBody, role);
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
        .expect("accepted indented Choice suite has an item indent");
    let mut indent_cursor = SuiteLineIndentCursor::new(interval.first_item(), suite_indent);
    let mut ordinal = 0_u32;
    while parser.cursor() < interval.end() {
        bump_trivia_before(parser, interval.end());
        if parser.cursor() >= interval.end() {
            break;
        }
        let start = parser.cursor();
        let continues_else = parser.current_text() == Some("if");
        let item_end = indented_item_end(
            parser,
            start,
            interval.end(),
            suite_indent,
            is_choice_item_head,
            |_, spelling| continues_else && spelling == Some("else"),
        );
        let significant_end = trimmed_end(parser, start, item_end);
        if indent_cursor.observe(parser, start) == suite_indent {
            let semantic_end = if matches!(parser.current_text(), Some("for" | "option")) {
                item_end
            } else {
                significant_end
            };
            emit_choice_item(parser, semantic_end, item_kind, ordinal);
        } else {
            emit_recovery(
                parser,
                significant_end,
                SyntaxRole::ChoiceItem(ordinal),
                "syntax.choice.invalid_item_indent",
                "Choice item indentation must match the first item",
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
            .expect("the grammar budget keeps Choice item ordinals within u32");
    }
    bump_until(parser, interval.end());
    parser.finish();
    parser.finish();
}

fn emit_choice_colon(parser: &mut ShadowDocumentParser<'_, '_>) {
    debug_assert!(parser.at(":"));
    parser.start(SyntaxKind::ColonNode, SyntaxRole::Colon);
    parser.bump();
    parser.finish();
}

fn emit_indented_suite_issue(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    issue: IndentedSuiteIssue,
) {
    let (code, message) = match issue {
        IndentedSuiteIssue::MissingNewline => (
            "syntax.choice.indented_missing_newline",
            "an indented Choice suite requires a newline after `:`",
        ),
        IndentedSuiteIssue::MissingIndentedItem => (
            "syntax.choice.indented_missing_item",
            "an indented Choice suite requires at least one indented item",
        ),
    };
    emit_recovery(parser, end, SyntaxRole::Recovery(0), code, message);
}

fn is_choice_item_head(kind: Option<SyntaxKind>, spelling: Option<&str>) -> bool {
    kind == Some(SyntaxKind::EntityReferenceToken)
        || spelling
            .is_some_and(|spelling| matches!(spelling, "let" | "if" | "for" | "match" | "option"))
}

fn nested_choice_body_introducer(
    parser: &ShadowDocumentParser<'_, '_>,
    owner_start: usize,
    limit: usize,
) -> Option<ChoiceBodyIntroducer> {
    let head_end = physical_line_end(parser, owner_start, limit);
    if let Some(index) =
        trailing_owner_body_token(parser, owner_start.saturating_add(1), head_end, true)
        && token_text(parser, index) == Some(":")
    {
        return Some(ChoiceBodyIntroducer::Indented(indented_suite_interval(
            parser,
            owner_start,
            index,
            limit,
        )));
    }
    if let Some(index) =
        trailing_owner_body_token(parser, owner_start.saturating_add(1), limit, false)
        && token_text(parser, index) == Some("{")
    {
        return Some(ChoiceBodyIntroducer::Braced(index));
    }
    None
}

const fn choice_body_start(body: Option<ChoiceBodyIntroducer>) -> Option<usize> {
    match body {
        Some(ChoiceBodyIntroducer::Braced(open)) => Some(open),
        Some(ChoiceBodyIntroducer::Indented(interval)) => Some(interval.colon()),
        None => None,
    }
}

fn emit_choice_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(SyntaxKind::ChoiceBody, role);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    let mut ordinal = 0_u32;

    while parser.cursor() < close {
        bump_trivia_before(parser, close);
        if parser.cursor() >= close {
            break;
        }
        let start = parser.cursor();
        let terminator = find_choice_item_terminator(parser, start, close);
        let segment_end = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, segment_end);
        if start == significant_end {
            bump_until(parser, segment_end);
            continue;
        }
        emit_choice_item(parser, significant_end, item_kind, ordinal);
        let consumed_end = if terminator.is_some_and(|(_, semicolon)| semicolon) {
            segment_end.saturating_add(1)
        } else {
            segment_end
        };
        bump_until(parser, consumed_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget keeps Choice item ordinals within u32");
    }

    finish_choice_delimited_body(
        parser,
        "syntax.choice.missing_block_close",
        "missing closing `}` for Choice body",
    );
}

/// Finds one braced Choice item boundary while keeping a next-line `else`
/// attached to the `if` that owns it.
fn find_choice_item_terminator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<(usize, bool)> {
    let mut terminator = find_statement_terminator(parser, start, end)?;
    if token_text(parser, start) != Some("if") {
        return Some(terminator);
    }
    let mut segment_start = start;
    loop {
        let segment = choice_if_segment_boundary(parser, segment_start, terminator.0);
        if let Some(duplicate) = segment.duplicate_else {
            return Some((duplicate, false));
        }
        if terminator.1 || !segment.accepts_following_else {
            break;
        }
        let Some(continuation) = first_significant(parser, terminator.0.saturating_add(1), end)
        else {
            break;
        };
        if token_text(parser, continuation) != Some("else") {
            break;
        }
        let Some(next) = find_statement_terminator(parser, continuation, end) else {
            return Some((end, false));
        };
        segment_start = continuation;
        terminator = next;
    }
    Some(terminator)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChoiceIfSegmentBoundary {
    accepts_following_else: bool,
    duplicate_else: Option<usize>,
}

/// Classifies the owner-level `else` tokens in one physical segment and
/// exposes a second `else` after a terminal branch as the next recoverable
/// Choice item boundary.
fn choice_if_segment_boundary(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> ChoiceIfSegmentBoundary {
    let mut depth = 0_usize;
    let mut terminal_else = false;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        let text = parser.text_of(token);
        if depth == 0 && text == "else" {
            if terminal_else {
                return ChoiceIfSegmentBoundary {
                    accepts_following_else: false,
                    duplicate_else: Some(index),
                };
            }
            let next = first_significant(parser, index.saturating_add(1), end)
                .and_then(|next| token_text(parser, next));
            if next != Some("if") {
                terminal_else = true;
            }
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    ChoiceIfSegmentBoundary {
        accepts_following_else: !terminal_else,
        duplicate_else: None,
    }
}

/// Collects the owner-level `else` separators for one already bounded Choice
/// item with a single forward scan. Branch emission then consumes this list
/// monotonically instead of rescanning the unconsumed tail for every branch.
fn choice_if_else_tokens(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Vec<usize> {
    let mut depth = 0_usize;
    let mut separators = Vec::new();
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        let text = parser.text_of(token);
        if depth == 0 && text == "else" {
            separators.push(index);
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    separators
}

fn emit_choice_item(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    match parser.current_text() {
        Some("let") => {
            emit_statement_with_role(parser, end, item_kind, SyntaxRole::ChoiceItem(ordinal));
        }
        Some("if") => emit_choice_if(parser, end, item_kind, SyntaxRole::ChoiceItem(ordinal)),
        Some("for") => emit_choice_for(parser, end, item_kind, ordinal),
        Some("match") => emit_choice_match(parser, end, item_kind, ordinal),
        Some("option") => emit_choice_option(parser, end, item_kind, ordinal),
        _ if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) => {
            emit_choice_compact_arm(parser, end, item_kind, ordinal);
        }
        _ => emit_recovery(
            parser,
            end,
            SyntaxRole::ChoiceItem(ordinal),
            "syntax.choice.invalid_item",
            "unknown Choice item",
        ),
    }
}

fn emit_choice_if(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(SyntaxKind::ChoiceIfItem, role);
    let else_tokens = choice_if_else_tokens(parser, parser.cursor(), end);
    let mut else_tokens = else_tokens.into_iter();
    let mut ordinal = 0_u32;
    loop {
        let owner_start = parser.cursor();
        let else_token = else_tokens.next();
        let then_end = else_token.unwrap_or(end);
        let body = trailing_braced_body_interval(parser, owner_start.saturating_add(1), then_end);
        let condition_end = body.map_or(then_end, |(open, _)| open);
        parser.start(SyntaxKind::ChoiceIfBranch, SyntaxRole::Branch(ordinal));
        parser.bump();
        bump_trivia_before(parser, condition_end);
        emit_item_expression(parser, condition_end, SyntaxRole::Condition, item_kind);
        bump_until(parser, condition_end);
        if let Some((_, body_end)) = body {
            emit_choice_body(parser, body_end, item_kind, SyntaxRole::ThenBranch);
        } else {
            emit_missing_body(
                parser,
                SyntaxRole::ThenBranch,
                "syntax.choice.if_missing_body",
                "missing Choice `if` body",
            );
        }
        bump_until(parser, then_end);
        parser.finish();
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget keeps Choice branch ordinals within u32");

        let Some(else_token) = else_token else {
            break;
        };
        bump_until(parser, else_token);
        parser.bump();
        bump_trivia_before(parser, end);
        if parser.at("if") {
            continue;
        }
        if parser.at("{") {
            emit_choice_body(parser, end, item_kind, SyntaxRole::ElseBranch);
        } else {
            emit_missing_body(
                parser,
                SyntaxRole::ElseBranch,
                "syntax.choice.else_missing_body",
                "missing Choice `else` body",
            );
        }
        break;
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_for(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    let owner_start = parser.cursor();
    let body = nested_choice_body_introducer(parser, owner_start, end);
    let header_end = choice_body_start(body).unwrap_or(end);
    parser.start(SyntaxKind::ChoiceForItem, SyntaxRole::ChoiceItem(ordinal));
    parser.bump();
    bump_trivia_before(parser, header_end);
    let in_token =
        top_level_operator(parser, parser.cursor(), header_end, "in").unwrap_or(header_end);
    emit_pattern(parser, in_token, SyntaxRole::Pattern);
    bump_until(parser, in_token);
    if parser.at("in") {
        parser.bump();
        bump_trivia_before(parser, header_end);
    } else {
        emit_missing_token_diagnostic(
            parser,
            "syntax.choice.for_missing_in",
            "missing `in` in Choice `for` item",
        );
    }
    emit_item_expression(parser, header_end, SyntaxRole::Initializer, item_kind);
    bump_until(parser, header_end);
    match body {
        Some(ChoiceBodyIntroducer::Braced(open)) => {
            bump_until(parser, open);
            emit_choice_body(parser, end, item_kind, SyntaxRole::Body);
        }
        Some(ChoiceBodyIntroducer::Indented(interval)) => {
            bump_until(parser, interval.colon());
            emit_indented_choice_body(parser, interval, item_kind, SyntaxRole::Body);
        }
        None => {
            emit_missing_body(
                parser,
                SyntaxRole::Body,
                "syntax.choice.for_missing_body",
                "missing Choice `for` body",
            );
        }
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_match(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    let owner_start = parser.cursor();
    let body = trailing_braced_body_interval(parser, owner_start.saturating_add(1), end);
    let scrutinee_end = body.map_or(end, |(open, _)| open);
    parser.start(SyntaxKind::ChoiceMatchItem, SyntaxRole::ChoiceItem(ordinal));
    parser.bump();
    bump_trivia_before(parser, scrutinee_end);
    emit_item_expression(parser, scrutinee_end, SyntaxRole::Scrutinee, item_kind);
    bump_until(parser, scrutinee_end);
    if let Some((_, body_end)) = body {
        emit_choice_match_body(parser, body_end, item_kind);
    } else {
        emit_missing_body(
            parser,
            SyntaxRole::Body,
            "syntax.choice.match_missing_body",
            "missing Choice `match` body",
        );
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_match_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) {
    parser.start(SyntaxKind::ChoiceBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        bump_trivia_before(parser, close);
        if parser.cursor() >= close {
            break;
        }
        let arm_end = find_match_arm_end(parser, parser.cursor(), close);
        emit_choice_match_arm(parser, arm_end, item_kind, ordinal);
        bump_until(parser, arm_end);
        if matches!(parser.current_text(), Some("," | ";")) {
            parser.bump();
        }
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget keeps Choice Match arm ordinals within u32");
    }
    finish_choice_delimited_body(
        parser,
        "syntax.choice.match_missing_close",
        "missing closing `}` for Choice Match body",
    );
}

fn emit_choice_match_arm(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(SyntaxKind::ChoiceMatchArm, SyntaxRole::MatchArm(ordinal));
    let arrow = top_level_operator(parser, parser.cursor(), end, "=>").unwrap_or(end);
    let guard = top_level_operator(parser, parser.cursor(), arrow, "when").unwrap_or(arrow);
    emit_pattern(parser, guard, SyntaxRole::Pattern);
    bump_until(parser, guard);
    if parser.at("when") {
        parser.bump();
        bump_trivia_before(parser, arrow);
        emit_expression(parser, arrow, SyntaxRole::Guard);
        bump_until(parser, arrow);
    }
    if parser.at("=>") {
        parser.bump();
        bump_trivia_before(parser, end);
        if parser.at("{") {
            emit_choice_body(parser, end, item_kind, SyntaxRole::Body);
        } else if parser.cursor() < end {
            emit_choice_item(parser, end, item_kind, 0);
        } else {
            emit_missing_body(
                parser,
                SyntaxRole::Body,
                "syntax.choice.match_arm_missing_body",
                "missing Choice Match arm body",
            );
        }
    } else {
        emit_missing_body(
            parser,
            SyntaxRole::Body,
            "syntax.choice.match_arm_missing_separator",
            "Choice Match arm requires `=>`",
        );
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_option(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    let owner_start = parser.cursor();
    let body = nested_choice_body_introducer(parser, owner_start, end);
    let header_end = choice_body_start(body).unwrap_or(end);
    let after_keyword = parser.cursor().saturating_add(1);
    let in_token = top_level_operator(parser, after_keyword, header_end, "in");
    if in_token.is_some() {
        emit_choice_option_for(parser, end, item_kind, ordinal, body, header_end, in_token);
        return;
    }

    parser.start(SyntaxKind::ChoiceOption, SyntaxRole::ChoiceItem(ordinal));
    parser.bump();
    bump_trivia_before(parser, header_end);
    emit_item_expression(parser, header_end, SyntaxRole::PublicId, item_kind);
    bump_until(parser, header_end);
    match body {
        Some(ChoiceBodyIntroducer::Braced(open)) => {
            bump_until(parser, open);
            emit_choice_option_body(parser, end, item_kind);
        }
        Some(ChoiceBodyIntroducer::Indented(interval)) => {
            bump_until(parser, interval.colon());
            emit_indented_choice_option_body(parser, interval, item_kind);
        }
        None => {
            emit_missing_body(
                parser,
                SyntaxRole::Body,
                "syntax.choice.option_missing_body",
                "missing Choice option body",
            );
        }
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_option_for(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
    body: Option<ChoiceBodyIntroducer>,
    header_end: usize,
    in_token: Option<usize>,
) {
    parser.start(SyntaxKind::ChoiceOptionFor, SyntaxRole::ChoiceItem(ordinal));
    parser.bump();
    bump_trivia_before(
        parser,
        in_token.expect("OptionFor dispatch requires an `in` token"),
    );
    let in_token = in_token.expect("OptionFor dispatch requires an `in` token");
    emit_pattern(parser, in_token, SyntaxRole::Pattern);
    bump_until(parser, in_token);
    parser.bump();
    bump_trivia_before(parser, header_end);
    emit_item_expression(parser, header_end, SyntaxRole::Initializer, item_kind);
    bump_until(parser, header_end);
    match body {
        Some(ChoiceBodyIntroducer::Braced(open)) => {
            bump_until(parser, open);
            emit_choice_option_body(parser, end, item_kind);
        }
        Some(ChoiceBodyIntroducer::Indented(interval)) => {
            bump_until(parser, interval.colon());
            emit_indented_choice_option_body(parser, interval, item_kind);
        }
        None => {
            emit_missing_body(
                parser,
                SyntaxRole::Body,
                "syntax.choice.option_for_missing_body",
                "missing Choice option-for body",
            );
        }
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_option_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) {
    parser.start(SyntaxKind::ChoiceOptionBody, SyntaxRole::Body);
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
        emit_choice_option_field(parser, significant_end, item_kind, ordinal);
        let consumed_end = if terminator.is_some_and(|(_, semicolon)| semicolon) {
            segment_end.saturating_add(1)
        } else {
            segment_end
        };
        bump_until(parser, consumed_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget keeps Choice option fields within u32");
    }
    finish_choice_delimited_body(
        parser,
        "syntax.choice.option_missing_close",
        "missing closing `}` for Choice option body",
    );
}

fn emit_indented_choice_option_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    interval: IndentedSuiteInterval,
    item_kind: SyntaxKind,
) {
    parser.start(SyntaxKind::ChoiceOptionBody, SyntaxRole::Body);
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
        .expect("accepted indented Choice option suite has an item indent");
    let mut indent_cursor = SuiteLineIndentCursor::new(interval.first_item(), suite_indent);
    let mut ordinal = 0_u32;
    while parser.cursor() < interval.end() {
        bump_trivia_before(parser, interval.end());
        if parser.cursor() >= interval.end() {
            break;
        }
        let start = parser.cursor();
        let field_end = indented_item_end(
            parser,
            start,
            interval.end(),
            suite_indent,
            is_choice_option_field_head,
            |_, _| false,
        );
        let significant_end = trimmed_end(parser, start, field_end);
        if indent_cursor.observe(parser, start) == suite_indent {
            emit_choice_option_field(parser, significant_end, item_kind, ordinal);
        } else {
            emit_recovery(
                parser,
                significant_end,
                SyntaxRole::ChoiceOptionField(ordinal),
                "syntax.choice.invalid_option_field_indent",
                "Choice option field indentation must match the first field",
            );
        }
        let consumed_end = if token_text(parser, field_end) == Some(";") {
            field_end.saturating_add(1).min(interval.end())
        } else {
            field_end
        };
        bump_until(parser, consumed_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget keeps Choice option fields within u32");
    }
    bump_until(parser, interval.end());
    parser.finish();
    parser.finish();
}

fn emit_choice_compact_arm(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ChoiceCompactArm,
        SyntaxRole::ChoiceItem(ordinal),
    );
    emit_entity_reference(parser, SyntaxRole::PublicId);
    bump_trivia_before(parser, end);
    if matches!(
        parser.current_kind(),
        Some(SyntaxKind::StringToken | SyntaxKind::RawStringToken)
    ) {
        let label_end = parser.cursor().saturating_add(1).min(end);
        emit_item_expression(parser, label_end, SyntaxRole::Label(0), item_kind);
        bump_until(parser, label_end);
    } else {
        emit_missing_expression(
            parser,
            SyntaxRole::Label(0),
            "syntax.choice.compact_missing_label",
            "compact Choice arm requires a string label",
        );
    }
    bump_trivia_before(parser, end);
    let thin = top_level_operator(parser, parser.cursor(), end, "->");
    let fat = top_level_operator(parser, parser.cursor(), end, "=>");
    let action = match (thin, fat) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(action), None) | (None, Some(action)) => Some(action),
        (None, None) => None,
    };
    let head_end = action.unwrap_or(end);
    if parser.at("if") {
        parser.bump();
        bump_trivia_before(parser, head_end);
        emit_item_expression(parser, head_end, SyntaxRole::Condition, item_kind);
        bump_until(parser, head_end);
    } else if parser.cursor() < head_end {
        emit_recovery(
            parser,
            head_end,
            SyntaxRole::Recovery(0),
            "syntax.choice.compact_invalid_condition",
            "compact Choice condition must begin with `if`",
        );
    }
    match action.and_then(|index| token_text(parser, index)) {
        Some("->") => {
            parser.start(SyntaxKind::ChoiceGotoAction, SyntaxRole::Plan);
            bump_until(parser, action.expect("known compact action"));
            parser.bump();
            bump_trivia_before(parser, end);
            if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
                emit_entity_reference(parser, SyntaxRole::Target);
            } else {
                emit_missing_expression(
                    parser,
                    SyntaxRole::Target,
                    "syntax.choice.compact_missing_target",
                    "compact Choice goto action requires an entity reference",
                );
            }
            bump_until(parser, end);
            parser.finish();
        }
        Some("=>") => {
            parser.start(SyntaxKind::ChoiceOutAction, SyntaxRole::Plan);
            bump_until(parser, action.expect("known compact action"));
            parser.bump();
            bump_trivia_before(parser, end);
            emit_item_expression(parser, end, SyntaxRole::Value, item_kind);
            bump_until(parser, end);
            parser.finish();
        }
        _ => emit_missing_expression(
            parser,
            SyntaxRole::Plan,
            "syntax.choice.compact_missing_action",
            "compact Choice arm requires `->` or `=>` action",
        ),
    }
    parser.finish();
}

fn finish_choice_delimited_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    code: &'static str,
    message: &'static str,
) {
    if parser.at("}") {
        emit_close_delimiter(parser, SyntaxKind::CloseBraceNode, "}", code);
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            code,
            SourceRange::new(at, at),
            message,
        )));
    }
    parser.finish();
}

fn emit_missing_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
    code: &'static str,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingBody, role);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        message,
    )));
}

fn emit_missing_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
    code: &'static str,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingExpression, role);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        message,
    )));
}

fn emit_recovery(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    code: &'static str,
    message: &'static str,
) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, role);
    bump_until(parser, end);
    parser.finish();
    let finish = parser.current_offset();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(start, finish),
        message,
    )));
}

fn emit_missing_token_diagnostic(
    parser: &mut ShadowDocumentParser<'_, '_>,
    code: &'static str,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        message,
    )));
}
