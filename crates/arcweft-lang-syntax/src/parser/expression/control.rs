//! Structured control-expression events over the shared full-source cursor.

use super::{CompletedNode, emit_expression};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::parser::document::ShadowDocumentParser;
use crate::parser::pattern::emit_pattern;
use crate::parser::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, first_significant, token_count,
    trimmed_end,
};
use crate::parser::statement::emit_braced_block;

pub(super) fn emit_block_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    _end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    emit_block(parser, role);
    CompletedNode { start_event }
}

pub(super) fn emit_if_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let let_keyword = first_significant(parser, parser.cursor() + 1, end)
        .and_then(|index| parser.token_at(index))
        .is_some_and(|token| parser.text_of(token) == "let");
    parser.start(
        if let_keyword {
            SyntaxKind::IfLetExpression
        } else {
            SyntaxKind::IfExpression
        },
        role,
    );
    parser.bump();
    parser.bump_trivia();
    if let_keyword && parser.at("let") {
        parser.bump();
        parser.bump_trivia();
        emit_if_let_head(parser, end);
    } else {
        emit_if_condition(parser, end);
    }
    emit_if_branches(parser, end);
    parser.finish();
    CompletedNode { start_event }
}

fn emit_if_condition(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let branch = find_expression_boundary(parser, parser.cursor(), end, &["{"]);
    emit_expression(parser, branch, SyntaxRole::Condition);
    bump_until(parser, branch);
}

fn emit_if_let_head(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let branch = find_expression_boundary(parser, parser.cursor(), end, &["{"]);
    let assignment = find_expression_boundary(parser, parser.cursor(), branch, &["="]);
    emit_pattern(parser, assignment, SyntaxRole::Pattern);
    bump_until(parser, assignment);
    if parser.at("=") {
        parser.bump();
        parser.bump_trivia();
    }
    let guard = find_expression_boundary(parser, parser.cursor(), branch, &["when"]);
    emit_expression(parser, guard, SyntaxRole::Scrutinee);
    bump_until(parser, guard);
    if parser.at("when") {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, branch, SyntaxRole::Guard);
        bump_until(parser, branch);
    }
}

fn emit_if_branches(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.bump_trivia();
    if parser.at("{") {
        emit_block(parser, SyntaxRole::ThenBranch);
    }
    parser.bump_trivia();
    if !parser.at("else") {
        return;
    }
    parser.bump();
    parser.bump_trivia();
    if parser.at("if") {
        emit_if_expression(parser, end, SyntaxRole::ElseBranch);
    } else if parser.at("{") {
        emit_block(parser, SyntaxRole::ElseBranch);
    } else {
        emit_expression(parser, end, SyntaxRole::ElseBranch);
    }
}

pub(super) fn emit_match_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::MatchExpression, role);
    parser.bump();
    parser.bump_trivia();
    let open = find_expression_boundary(parser, parser.cursor(), end, &["{"]);
    emit_expression(parser, open, SyntaxRole::Scrutinee);
    bump_until(parser, open);
    if !parser.at("{") {
        parser.start(SyntaxKind::MatchArmList, SyntaxRole::Element(0));
        parser.finish();
        parser.finish();
        return CompletedNode { start_event };
    }

    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = crate::parser::shadow_recovery::find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or_else(|| token_count(parser))
        .min(end);
    parser.start(SyntaxKind::MatchArmList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("}") {
            break;
        }
        emit_match_arm(parser, close, ordinal);
        ordinal = ordinal.saturating_add(1);
        parser.bump_trivia();
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.expression.missing_match_close",
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_match_arm(parser: &mut ShadowDocumentParser<'_, '_>, close: usize, ordinal: u16) {
    parser.start(SyntaxKind::MatchArm, SyntaxRole::MatchArm(ordinal));
    let arrow = find_expression_boundary(parser, parser.cursor(), close, &["=>"]);
    let guard = find_expression_boundary(parser, parser.cursor(), arrow, &["when"]);
    emit_pattern(parser, guard, SyntaxRole::Pattern);
    bump_until(parser, guard);
    if parser.at("when") {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, arrow, SyntaxRole::Guard);
        bump_until(parser, arrow);
    }
    if parser.at("=>") {
        parser.bump();
        parser.bump_trivia();
    }
    if parser.at("{") {
        emit_block(parser, SyntaxRole::Body);
    } else {
        let value_end = match_arm_value_end(parser, close);
        emit_expression(parser, value_end, SyntaxRole::Body);
        bump_until(parser, value_end);
    }
    parser.finish();
}

fn match_arm_value_end(parser: &ShadowDocumentParser<'_, '_>, close: usize) -> usize {
    let mut depth = 0_usize;
    for index in parser.cursor()..close {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        let text = parser.text_of(token);
        if depth == 0 && (text == "," || token.kind() == SyntaxKind::NewlineToken || text == "}") {
            return trimmed_end(parser, parser.cursor(), index);
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    trimmed_end(parser, parser.cursor(), close)
}

fn find_expression_boundary(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    boundaries: &[&str],
) -> usize {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return index;
        };
        let text = parser.text_of(token);
        if depth == 0 && boundaries.contains(&text) {
            return index;
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    end
}

fn emit_block(parser: &mut ShadowDocumentParser<'_, '_>, role: SyntaxRole) {
    parser.start(SyntaxKind::BlockExpression, role);
    emit_braced_block(
        parser,
        SyntaxKind::FunctionItem,
        SyntaxKind::Block,
        SyntaxRole::Body,
        "syntax.expression.missing_block_close",
    );
    parser.finish();
}
