//! Structured control-expression events over the shared full-source cursor.

use arcweft_source::SourceRange;

use super::{
    CompletedNode, completed_slot, emit_expression, emit_expression_node, emit_missing_expression,
    expression_slot,
};
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
    PendingExpressionProjection, SyntaxExpressionSlot, SyntaxMatchArmPart,
    SyntaxMatchArmProjection, SyntaxMatchBodyTerminator, SyntaxMatchProjection,
    SyntaxRequiredTokenState,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::parser::cursor::DocumentParser;
use crate::parser::pattern::emit_pattern;
use crate::parser::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    find_matching_close, first_significant, trimmed_end,
};
use crate::parser::statement::emit_braced_block;

pub(super) fn emit_block_expression(
    parser: &mut DocumentParser<'_, '_>,
    _end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::BlockExpression, role);
    emit_block_contents(parser, SyntaxRole::Body);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::Block, Vec::new()),
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_unbraced_block_expression(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::BlockExpression, role);
    crate::parser::statement::emit_unbraced_block_until(
        parser,
        end,
        SyntaxKind::FunctionItem,
        SyntaxKind::Block,
        SyntaxRole::Body,
    );
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::Block, Vec::new()),
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_if_expression(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let let_keyword = first_significant(parser, parser.cursor() + 1, end)
        .and_then(|index| parser.token_at(index))
        .is_some_and(|token| parser.text_of(token) == "let");
    let owner = parser.start_projected_owner(
        if let_keyword {
            SyntaxKind::IfLetExpression
        } else {
            SyntaxKind::IfExpression
        },
        role,
    );
    parser.bump();
    parser.bump_trivia_before(end);
    if let_keyword {
        if parser.at("let") {
            parser.bump();
            parser.bump_trivia_before(end);
        }
        let head = emit_if_let_head(parser, end);
        let branches = emit_if_branches(parser, end);
        let mut components = vec![
            PendingExpressionComponent::new(ExpressionComponentRole::Pattern, head.pattern_range),
            PendingExpressionComponent::new(
                ExpressionComponentRole::Scrutinee,
                head.scrutinee_range,
            ),
            PendingExpressionComponent::new(
                ExpressionComponentRole::ThenBranch,
                branches.then_range,
            ),
            PendingExpressionComponent::new(
                ExpressionComponentRole::ElseBranch,
                branches.else_range,
            ),
        ];
        if let Some((_, range)) = head.guard {
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::Guard,
                range,
            ));
        }
        parser.set_expression_projection(
            owner,
            PendingExpressionProjection::new(
                ExpressionProjection::IfLet {
                    scrutinee: head.scrutinee,
                    guard: head.guard.map(|(slot, _)| slot),
                    then_branch: branches.then_branch,
                    else_branch: branches.else_branch,
                },
                components,
            ),
        );
    } else {
        let (condition, condition_range) = emit_if_condition(parser, end);
        let branches = emit_if_branches(parser, end);
        parser.set_expression_projection(
            owner,
            PendingExpressionProjection::new(
                ExpressionProjection::If {
                    condition,
                    then_branch: branches.then_branch,
                    else_branch: branches.else_branch,
                },
                vec![
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::Condition,
                        condition_range,
                    ),
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::ThenBranch,
                        branches.then_range,
                    ),
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::ElseBranch,
                        branches.else_range,
                    ),
                ],
            ),
        );
    }
    parser.finish();
    CompletedNode { start_event }
}

fn emit_if_condition(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
) -> (SyntaxExpressionSlot, SourceRange) {
    let branch = find_expression_boundary(parser, parser.cursor(), end, &["{", "else"]);
    let condition = expression_slot(parser, branch);
    emit_expression(parser, branch, SyntaxRole::Condition);
    bump_until(parser, branch);
    condition
}

struct IfLetHead {
    pattern_range: SourceRange,
    scrutinee: SyntaxExpressionSlot,
    scrutinee_range: SourceRange,
    guard: Option<(SyntaxExpressionSlot, SourceRange)>,
}

fn emit_if_let_head(parser: &mut DocumentParser<'_, '_>, end: usize) -> IfLetHead {
    let branch = find_expression_boundary(parser, parser.cursor(), end, &["{", "else"]);
    let assignment = find_expression_boundary(parser, parser.cursor(), branch, &["="]);
    let pattern_start = parser.event_position();
    emit_pattern(parser, assignment, SyntaxRole::Pattern);
    let pattern_range = parser
        .completed_range(pattern_start)
        .expect("if-let pattern retains one exact source range");
    bump_until(parser, assignment);
    if parser.at("=") {
        parser.bump();
        parser.bump_trivia_before(branch);
    }
    let guard = find_expression_boundary(parser, parser.cursor(), branch, &["when"]);
    let (scrutinee, scrutinee_range) = expression_slot(parser, guard);
    emit_expression(parser, guard, SyntaxRole::Scrutinee);
    bump_until(parser, guard);
    let guard = if parser.at("when") {
        parser.bump();
        parser.bump_trivia_before(branch);
        let guard = expression_slot(parser, branch);
        emit_expression(parser, branch, SyntaxRole::Guard);
        bump_until(parser, branch);
        Some(guard)
    } else {
        None
    };
    IfLetHead {
        pattern_range,
        scrutinee,
        scrutinee_range,
        guard,
    }
}

struct IfBranches {
    then_branch: SyntaxExpressionSlot,
    then_range: SourceRange,
    else_branch: Option<SyntaxExpressionSlot>,
    else_range: SourceRange,
}

fn emit_if_branches(parser: &mut DocumentParser<'_, '_>, end: usize) -> IfBranches {
    parser.bump_trivia_before(end);
    let then_branch = if parser.at("{") {
        emit_block(parser, SyntaxRole::ThenBranch)
    } else {
        emit_missing_expression(parser, SyntaxRole::ThenBranch)
    };
    let then_slot = completed_slot(parser, then_branch);
    let then_range = parser
        .completed_range(then_branch.start_event)
        .expect("if then branch retains one exact source range");
    parser.bump_trivia_before(end);
    if !parser.at("else") {
        let insertion = SourceRange::new(then_range.end(), then_range.end());
        return IfBranches {
            then_branch: then_slot,
            then_range,
            else_branch: None,
            else_range: insertion,
        };
    }
    parser.bump();
    parser.bump_trivia_before(end);
    let else_branch = if parser.at("if") {
        emit_if_expression(parser, end, SyntaxRole::ElseBranch)
    } else if parser.at("{") {
        emit_block(parser, SyntaxRole::ElseBranch)
    } else {
        emit_expression_node(parser, end, SyntaxRole::ElseBranch)
    };
    let else_slot = completed_slot(parser, else_branch);
    let else_range = parser
        .completed_range(else_branch.start_event)
        .expect("if else branch retains one exact source range");
    IfBranches {
        then_branch: then_slot,
        then_range,
        else_branch: Some(else_slot),
        else_range,
    }
}

pub(super) fn emit_match_expression(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::MatchExpression, role);
    parser.bump();
    parser.bump_trivia_before(end);
    let open = find_expression_boundary(parser, parser.cursor(), end, &["{"]);
    let (scrutinee, scrutinee_range) = expression_slot(parser, open);
    emit_expression(parser, open, SyntaxRole::Scrutinee);
    bump_until(parser, open);
    if !parser.at("{") {
        parser.start(SyntaxKind::MatchArmList, SyntaxRole::Element(0));
        parser.finish();
        parser.set_expression_projection(
            owner,
            PendingExpressionProjection::new(
                ExpressionProjection::Match(SyntaxMatchProjection::new(
                    scrutinee,
                    Vec::new(),
                    SyntaxMatchBodyTerminator::MissingBody,
                )),
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::Scrutinee,
                    scrutinee_range,
                )],
            ),
        );
        parser.finish();
        return CompletedNode { start_event };
    }

    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let matched_close =
        find_matching_close(parser, parser.cursor(), "{").filter(|close| *close < end);
    let close = matched_close.unwrap_or(end).min(end);
    parser.start(SyntaxKind::MatchArmList, SyntaxRole::Element(0));
    let mut arms = Vec::new();
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::Scrutinee,
        scrutinee_range,
    )];
    loop {
        parser.bump_trivia_before(close);
        if parser.cursor() >= close || parser.at("}") {
            break;
        }
        let ordinal = u32::try_from(arms.len())
            .expect("the expression grammar budget keeps Match arm ordinals within u32");
        let arm = emit_match_arm(parser, close, ordinal);
        arms.push(arm.projection);
        components.extend(arm.components);
        parser.bump_trivia_before(close);
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    let terminator = if matched_close.is_some() {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.expression.missing_match_close",
        );
        SyntaxMatchBodyTerminator::Closed
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        SyntaxMatchBodyTerminator::RecoveredMissingClose
    };
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Match(SyntaxMatchProjection::new(scrutinee, arms, terminator)),
            components,
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

struct EmittedMatchArm {
    projection: SyntaxMatchArmProjection,
    components: Vec<PendingExpressionComponent>,
}

fn emit_match_arm(
    parser: &mut DocumentParser<'_, '_>,
    close: usize,
    ordinal: u32,
) -> EmittedMatchArm {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::MatchArm, SyntaxRole::MatchArm(ordinal));
    let arrow = find_expression_boundary(parser, parser.cursor(), close, &["=>"]);
    let guard = find_expression_boundary(parser, parser.cursor(), arrow, &["when"]);
    let pattern_start = parser.event_position();
    emit_pattern(parser, guard, SyntaxRole::Pattern);
    let pattern_range = parser
        .completed_range(pattern_start)
        .expect("a Match arm Pattern retains one exact source range");
    bump_until(parser, guard);
    let guard = if parser.at("when") {
        parser.bump();
        parser.bump_trivia_before(arrow);
        let guard = emit_expression_node(parser, arrow, SyntaxRole::Guard);
        let slot = completed_slot(parser, guard);
        let range = parser
            .completed_range(guard.start_event)
            .expect("a Match arm Guard retains one exact source range");
        bump_until(parser, arrow);
        Some((slot, range))
    } else {
        None
    };
    let (arrow_state, arrow_range) = if parser.at("=>") {
        let range = parser
            .current()
            .expect("an authored Match arrow retains its token")
            .range();
        parser.bump();
        (SyntaxRequiredTokenState::Present, range)
    } else {
        let at = parser.current_offset();
        (SyntaxRequiredTokenState::Missing, SourceRange::new(at, at))
    };
    let value_end = match_arm_value_end(parser, close);
    parser.bump_trivia_before(value_end);
    let value = if parser.at("{") {
        emit_block(parser, SyntaxRole::Body)
    } else {
        let value = emit_expression_node(parser, value_end, SyntaxRole::Body);
        bump_until(parser, value_end);
        value
    };
    let value_slot = completed_slot(parser, value);
    let value_range = parser
        .completed_range(value.start_event)
        .expect("a Match arm Value retains one exact source range");
    parser.finish();
    let whole_range = parser
        .completed_range(start_event)
        .expect("a Match arm retains one exact source range");
    let mut components = vec![
        PendingExpressionComponent::new(
            ExpressionComponentRole::MatchArm {
                arm: ordinal,
                part: SyntaxMatchArmPart::Whole,
            },
            whole_range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::MatchArm {
                arm: ordinal,
                part: SyntaxMatchArmPart::Pattern,
            },
            pattern_range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::MatchArm {
                arm: ordinal,
                part: SyntaxMatchArmPart::Arrow,
            },
            arrow_range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::MatchArm {
                arm: ordinal,
                part: SyntaxMatchArmPart::Value,
            },
            value_range,
        ),
    ];
    if let Some((_, range)) = guard {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::MatchArm {
                arm: ordinal,
                part: SyntaxMatchArmPart::Guard,
            },
            range,
        ));
    }
    EmittedMatchArm {
        projection: SyntaxMatchArmProjection::new(
            guard.map(|(slot, _)| slot),
            arrow_state,
            value_slot,
        ),
        components,
    }
}

fn match_arm_value_end(parser: &DocumentParser<'_, '_>, close: usize) -> usize {
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
    parser: &DocumentParser<'_, '_>,
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

fn emit_block(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::BlockExpression, role);
    emit_block_contents(parser, SyntaxRole::Body);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::Block, Vec::new()),
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_block_contents(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) {
    emit_braced_block(
        parser,
        SyntaxKind::FunctionItem,
        SyntaxKind::Block,
        role,
        "syntax.expression.missing_block_close",
    );
}
