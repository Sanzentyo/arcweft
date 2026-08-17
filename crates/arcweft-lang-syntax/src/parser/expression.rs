//! Private Pratt expression grammar over the shared document cursor.

mod call_arguments;
mod colon_dialogue;
mod composite;
mod control;
mod operators;
mod postfix_bracket;

pub(super) use call_arguments::emit_parenthesized_call_tail;
pub(in crate::parser) use colon_dialogue::emit_colon_dialogue_application;
pub(in crate::parser) use composite::expression_slot;

use self::operators::{binary_binding_power, is_postfix_operator, syntax_binary_operator};
use self::postfix_bracket::emit_postfix_bracket;

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::lexer::{
    LiteralLexemePart, typed_entity_reference, typed_lifetime_registry_path, typed_literal,
};
use super::path::{PathSeparatorGrammar, emit_path};
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    find_matching_close, find_statement_terminator, find_top_level_boundary, trimmed_end,
};
use super::type_ref::{
    PreparedTypeProjection, emit_prepared_type, emit_recovered_type, prepare_type,
};
use crate::expressions::{
    ExpressionComponentRole, ExpressionLiteralPart, ExpressionProjection,
    PendingExpressionComponent, PendingExpressionProjection, SyntaxAssociatedCallSyntax,
    SyntaxAssociatedReceiver, SyntaxAssociatedSeparator, SyntaxBinaryOperator, SyntaxBorrowKind,
    SyntaxCallArgumentListTerminator, SyntaxCallArgumentPart, SyntaxCallArgumentProjection,
    SyntaxCallProjection, SyntaxCallTypeApplicationComponentRole,
    SyntaxCallTypeApplicationProjection, SyntaxCallTypeApplicationSpelling,
    SyntaxCallTypeApplicationTerminator, SyntaxCallTypeArgumentPart,
    SyntaxCallTypeArgumentProjection, SyntaxCallbackBlockCallProjection, SyntaxClosureProjection,
    SyntaxClosureSyntax, SyntaxClosureTerminator, SyntaxExpressionSlot,
    SyntaxParenthesizedCallProjection, SyntaxPlaceholderKind, SyntaxRequiredTokenState,
    SyntaxSelectedMember, SyntaxUnaryOperator,
};
use crate::grammar::keyword_statement_projection::{
    PendingAwaitBranchProjection, SyntaxAwaitBranchKind,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::name::{SyntaxName, SyntaxNameIssue};
use crate::types::TypeRef;

/// Whether a token continues an already completed expression at owner level.
///
/// Statement grammars use this closed Pratt vocabulary when distinguishing a
/// braced expression head from a following statement-owned body. Keeping the
/// classification here prevents those owners from growing parallel operator
/// tables.
pub(in crate::parser) fn is_expression_continuation_token(spelling: &str) -> bool {
    is_postfix_operator(spelling) || binary_binding_power(spelling).is_some()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CompletedNode {
    pub(super) start_event: usize,
}

pub(super) fn emit_expression(parser: &mut DocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    let _ = emit_expression_node(parser, end, role);
}

pub(super) fn emit_expression_node(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let end = trimmed_end(parser, parser.cursor(), end);
    if parser.cursor() >= end {
        return emit_missing_expression(parser, role);
    }

    let completed = parse_binding_power(parser, end, 0, role);
    if parser.cursor() < end {
        let recovery_start = parser
            .current()
            .expect("unconsumed expression suffix has a first token")
            .range()
            .start();
        let recovery_end = parser
            .token_at(end - 1)
            .expect("trimmed expression suffix has a final token")
            .range()
            .end();
        let owner =
            parser.insert_projected_start(completed.start_event, SyntaxKind::ErrorExpression, role);
        parser.set_start_role(completed.start_event + 1, SyntaxRole::Operand);
        parser.set_expression_projection(
            owner,
            PendingExpressionProjection::new(
                ExpressionProjection::Error,
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::Recovery,
                    SourceRange::new(recovery_start, recovery_end),
                )],
            ),
        );
        bump_until(parser, end);
        parser.finish();
    }
    completed
}

fn emit_missing_expression(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::MissingExpression, role);
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn completed_slot(
    parser: &DocumentParser<'_, '_>,
    expression: CompletedNode,
) -> SyntaxExpressionSlot {
    if parser.completed_kind(expression.start_event) == Some(SyntaxKind::MissingExpression) {
        SyntaxExpressionSlot::Missing
    } else {
        SyntaxExpressionSlot::Authored
    }
}

/// Emits one owner-provided named plan section through the shared expression
/// block grammar without teaching ordinary expression dispatch owner names.
pub(super) fn emit_named_plan_block(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    composite::emit_owner_named_block(parser, end, role);
}

pub(super) fn expression_is_call(
    parser: &DocumentParser<'_, '_>,
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
            "." | "::" => {}
            _ if token.kind() == SyntaxKind::IdentifierToken => {}
            _ => return false,
        }
    }
    false
}

fn parse_binding_power(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    minimum: u8,
    role: SyntaxRole,
) -> CompletedNode {
    let mut left = parse_prefix(parser, end, role);

    while let Some((operator_index, _, operator)) = parser.next_significant() {
        if operator_index >= end {
            break;
        }

        if let Some(application) =
            prepare_terminal_call_type_application(parser, end, left, operator_index)
        {
            bump_until(parser, application.start);
            left = emit_typed_call(parser, end, left, role, application);
            continue;
        }

        if operator == "{"
            && parser.completed_kind(left.start_event) == Some(SyntaxKind::SelectExpression)
        {
            bump_until(parser, operator_index);
            left = emit_callback_block_call(parser, end, left, role);
            continue;
        }

        if operator == "?" {
            bump_until(parser, operator_index);
            left = emit_rejected_postfix_question(parser, left, role);
            continue;
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

        let operator_range = parser
            .token_at(operator_index)
            .expect("binary dispatch retains its operator token")
            .range();
        if kind == SyntaxKind::RangeExpression {
            left = emit_infix_range(
                parser,
                end,
                left,
                role,
                InfixRangeOperator {
                    index: operator_index,
                    range: operator_range,
                    right_power,
                    inclusive: operator == "..=",
                },
            );
            continue;
        }
        left = emit_binary_expression(
            parser,
            end,
            left,
            role,
            InfixBinaryOperator {
                index: operator_index,
                range: operator_range,
                right_power,
                kind,
                operator: syntax_binary_operator(operator),
            },
        );
    }

    left
}

fn emit_rejected_postfix_question(
    parser: &mut DocumentParser<'_, '_>,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    let question = parser
        .current()
        .expect("rejected postfix question dispatch retains its token")
        .range();
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::ErrorExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Operand);
    parser.bump();
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Error,
            vec![PendingExpressionComponent::new(
                ExpressionComponentRole::Recovery,
                question,
            )],
        ),
    );
    parser.finish();
    left
}

fn emit_binary_expression(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
    operator: InfixBinaryOperator,
) -> CompletedNode {
    let left_range = parser
        .completed_range(left.start_event)
        .expect("completed left expression retains one exact source range");
    bump_until(parser, operator.index);
    let projected = matches!(
        operator.kind,
        SyntaxKind::PipeExpression | SyntaxKind::BinaryExpression
    );
    let owner = if projected {
        parser.insert_projected_start(left.start_event, operator.kind, role)
    } else {
        parser.insert_start(left.start_event, operator.kind, role);
        None
    };
    parser.set_start_role(left.start_event + 1, SyntaxRole::LeftOperand);
    parser.bump();
    parser.bump_trivia_before(end);
    let right = if parser.cursor() < end {
        parse_binding_power(parser, end, operator.right_power, SyntaxRole::RightOperand)
    } else {
        emit_missing_expression(parser, SyntaxRole::RightOperand)
    };
    if projected {
        emit_binary_projection(parser, owner, operator, left_range, right);
    }
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn emit_binary_projection(
    parser: &mut DocumentParser<'_, '_>,
    owner: Option<usize>,
    operator: InfixBinaryOperator,
    left_range: SourceRange,
    right: CompletedNode,
) {
    let right_range = parser
        .completed_range(right.start_event)
        .expect("completed right expression retains one exact source range");
    let right_slot = completed_slot(parser, right);
    let projection = if operator.kind == SyntaxKind::PipeExpression {
        ExpressionProjection::Pipe([SyntaxExpressionSlot::Authored, right_slot])
    } else {
        ExpressionProjection::Binary {
            left: SyntaxExpressionSlot::Authored,
            operator: operator
                .operator
                .expect("binary binding-power dispatch uses the closed operator set"),
            right: right_slot,
        }
    };
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            projection,
            vec![
                PendingExpressionComponent::new(ExpressionComponentRole::LeftOperand, left_range),
                PendingExpressionComponent::new(ExpressionComponentRole::Operator, operator.range),
                PendingExpressionComponent::new(ExpressionComponentRole::RightOperand, right_range),
            ],
        ),
    );
}

fn parse_prefix(
    parser: &mut DocumentParser<'_, '_>,
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

    if text == "choice" {
        return super::statement::choice::emit_choice_expression(parser, end, role);
    }

    if matches!(
        token.kind(),
        SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
    ) && !matches!(
        text,
        "choice"
            | "try"
            | "await"
            | "thread"
            | "result"
            | "task"
            | "seq"
            | "stream"
            | "scope"
            | "loop"
            | "if"
            | "match"
            | "true"
            | "false"
    ) && let Some(call) = emit_associated_call(parser, end, role)
    {
        return call;
    }

    match text {
        "&" => emit_prefix_operand(parser, end, SyntaxKind::BorrowExpression, role, true),
        "*" => emit_prefix_operand(parser, end, SyntaxKind::DereferenceExpression, role, false),
        "!" | "-" => emit_prefix_operand(parser, end, SyntaxKind::UnaryExpression, role, false),
        ".." | "..=" => emit_prefix_range(parser, end, role, text == "..="),
        "try" => emit_prefix_operand(parser, end, SyntaxKind::TryExpression, role, false),
        "await" => emit_prefix_operand(parser, end, SyntaxKind::AwaitExpression, role, false),
        "thread" if composite::has_braced_body(parser, end) => {
            composite::emit_thread_expression(parser, end, role)
        }
        "result" | "task" | "seq" | "stream" if composite::has_braced_body(parser, end) => {
            composite::emit_computation_block(parser, end, role)
        }
        "scope" if composite::has_braced_body(parser, end) => {
            composite::emit_named_block(parser, end, role)
        }
        "loop" => control::emit_loop_expression(parser, end, role),
        "(" => composite::emit_parenthesized(parser, end, role),
        "[" => composite::emit_bracket_sequence(parser, end, role),
        "." => emit_short_variant(parser, end, role),
        "{" => composite::emit_braced_expression(parser, end, role),
        "if" => control::emit_if_expression(parser, end, role),
        "match" => control::emit_match_expression(parser, end, role),
        "|" | "||" => composite::emit_closure(parser, end, role),
        _ if syntax_binary_operator(text).is_some() => {
            emit_missing_left_binary(parser, end, role, text)
        }
        "_" | "^" => emit_placeholder(parser, role),
        "true" | "false" => emit_literal(parser, role),
        _ if token.kind() == SyntaxKind::EntityReferenceToken => {
            emit_entity_reference(parser, role).0
        }
        _ if token.kind() == SyntaxKind::LifetimeToken => emit_lifetime_path(parser, role),
        _ if is_literal(token.kind()) => emit_literal(parser, role),
        _ if matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        ) && composite::is_nominal_record_head(parser, end) =>
        {
            composite::emit_record_expression(parser, end, role)
        }
        _ if matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        ) =>
        {
            emit_path_expression(parser, end, role)
        }
        _ => emit_error(parser, role),
    }
}

fn emit_prefix_operand(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
    accepts_mutability: bool,
) -> CompletedNode {
    let start_event = parser.event_position();
    if !parser.enter_prefix_expression() {
        bump_until(parser, end);
        return CompletedNode { start_event };
    }
    let mut operator_range = parser
        .current()
        .expect("prefix dispatch retains one operator token")
        .range();
    let unary_operator = match parser.current_text() {
        Some("!") => Some(SyntaxUnaryOperator::Not),
        Some("-") => Some(SyntaxUnaryOperator::Negate),
        _ => None,
    };
    let owner = parser.start_projected_owner(kind, role);
    parser.bump();
    parser.bump_trivia_before(end);
    let mut borrow_kind = SyntaxBorrowKind::Shared;
    if accepts_mutability && parser.at("mut") {
        let mutable = parser
            .current()
            .expect("borrow mutability dispatch retains its token")
            .range();
        operator_range = SourceRange::new(operator_range.start(), mutable.end());
        borrow_kind = SyntaxBorrowKind::Mutable;
        parser.bump();
        parser.bump_trivia_before(end);
    }
    let await_with = (kind == SyntaxKind::AwaitExpression)
        .then(|| top_level_with(parser, parser.cursor(), end))
        .flatten();
    let operand_end = await_with.unwrap_or(end);
    let operand = if parser.cursor() < operand_end {
        parse_binding_power(parser, operand_end, 90, SyntaxRole::Operand)
    } else {
        emit_missing_expression(parser, SyntaxRole::Operand)
    };
    if parser.budget_failed() {
        parser.leave_prefix_expression();
        return CompletedNode { start_event };
    }
    let operand_range = parser
        .completed_range(operand.start_event)
        .expect("completed prefix operand retains one exact source range");
    let operand_slot = completed_slot(parser, operand);
    let branches = emit_await_branches(parser, end, kind, await_with);
    let projection = match kind {
        SyntaxKind::TryExpression => ExpressionProjection::Try {
            operand: operand_slot,
        },
        SyntaxKind::AwaitExpression => ExpressionProjection::Await {
            operand: operand_slot,
            branches: branches.as_ref().map(|(branches, _)| branches.clone()),
        },
        SyntaxKind::BorrowExpression => ExpressionProjection::Borrow {
            operand: operand_slot,
            kind: borrow_kind,
        },
        SyntaxKind::DereferenceExpression => ExpressionProjection::Dereference {
            operand: operand_slot,
        },
        SyntaxKind::UnaryExpression => ExpressionProjection::Unary {
            operand: operand_slot,
            operator: unary_operator.expect("unary dispatch is closed over `!` and `-`"),
        },
        _ => unreachable!("prefix operand projection kind is closed"),
    };
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(projection, {
            let mut components = vec![
                PendingExpressionComponent::new(ExpressionComponentRole::Operator, operator_range),
                PendingExpressionComponent::new(ExpressionComponentRole::Operand, operand_range),
            ];
            if let Some((_, range)) = branches {
                components.push(PendingExpressionComponent::new(
                    ExpressionComponentRole::AwaitWith,
                    range,
                ));
            }
            components
        }),
    );
    parser.finish();
    parser.leave_prefix_expression();
    CompletedNode { start_event }
}

fn emit_await_branches(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    await_with: Option<usize>,
) -> Option<(Box<[Option<SyntaxAwaitBranchKind>]>, SourceRange)> {
    if kind != SyntaxKind::AwaitExpression {
        return None;
    }
    let with = await_with?;
    bump_until(parser, with);
    let with_range = parser
        .current()
        .expect("await with dispatch retains its `with` token")
        .range();
    parser.bump();
    parser.bump_trivia_before(end);
    let branches = if parser.at("{") {
        super::statement::emit_await_with_branch_block(parser, end, SyntaxKind::FunctionItem)
            .into_iter()
            .map(PendingAwaitBranchProjection::kind)
            .collect::<Box<[_]>>()
    } else if parser.at(":") {
        let interval = super::statement::await_with_indented_suite_interval(parser, with, end);
        super::statement::emit_await_with_indented_branch_block(
            parser,
            interval,
            SyntaxKind::FunctionItem,
        )
        .into_iter()
        .map(PendingAwaitBranchProjection::kind)
        .collect::<Box<[_]>>()
    } else {
        super::statement::emit_required_statement_body_recovery(
            parser,
            "syntax.await_with.missing_body",
            "missing Await branch body",
        );
        Box::new([])
    };
    Some((branches, with_range))
}

fn top_level_with(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for index in start..end {
        let token = parser.token_at(index)?;
        let text = parser.text_of(token);
        if depth == 0 && text == "with" {
            return Some(index);
        }
        match text {
            "(" | "[" | "{" => depth = depth.saturating_add(1),
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

#[derive(Clone, Copy)]
struct InfixBinaryOperator {
    index: usize,
    range: SourceRange,
    right_power: u8,
    kind: SyntaxKind,
    operator: Option<SyntaxBinaryOperator>,
}

#[derive(Clone, Copy)]
struct InfixRangeOperator {
    index: usize,
    range: SourceRange,
    right_power: u8,
    inclusive: bool,
}

fn emit_infix_range(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
    operator: InfixRangeOperator,
) -> CompletedNode {
    let left_range = parser
        .completed_range(left.start_event)
        .expect("completed range start retains one exact source range");
    bump_until(parser, operator.index);
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::RangeExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::LeftOperand);
    parser.bump();
    parser.bump_trivia_before(end);
    let right = (parser.cursor() < end)
        .then(|| parse_binding_power(parser, end, operator.right_power, SyntaxRole::RightOperand));
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::RangeStart,
        left_range,
    )];
    if operator.inclusive {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RangeInclusiveMarker,
            operator.range,
        ));
    }
    let end_slot = right.map(|right| {
        let right_range = parser
            .completed_range(right.start_event)
            .expect("completed range end retains one exact source range");
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RangeEnd,
            right_range,
        ));
        completed_slot(parser, right)
    });
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Range {
                start: Some(SyntaxExpressionSlot::Authored),
                end: end_slot,
                inclusive: operator.inclusive,
            },
            components,
        ),
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn emit_prefix_range(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    inclusive: bool,
) -> CompletedNode {
    let start_event = parser.event_position();
    if !parser.enter_prefix_expression() {
        bump_until(parser, end);
        return CompletedNode { start_event };
    }
    let operator_range = parser
        .current()
        .expect("prefix range dispatch retains its operator token")
        .range();
    let owner = parser.start_projected_owner(SyntaxKind::RangeExpression, role);
    parser.bump();
    parser.bump_trivia_before(end);
    let end_expression = (parser.cursor() < end)
        .then(|| parse_binding_power(parser, end, 12, SyntaxRole::RightOperand));
    if parser.budget_failed() {
        parser.leave_prefix_expression();
        return CompletedNode { start_event };
    }
    let mut components =
        Vec::with_capacity(usize::from(inclusive) + usize::from(end_expression.is_some()));
    if inclusive {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RangeInclusiveMarker,
            operator_range,
        ));
    }
    let end_slot = end_expression.map(|end_expression| {
        let end_range = parser
            .completed_range(end_expression.start_event)
            .expect("completed range end retains one exact source range");
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RangeEnd,
            end_range,
        ));
        completed_slot(parser, end_expression)
    });
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Range {
                start: None,
                end: end_slot,
                inclusive,
            },
            components,
        ),
    );
    parser.finish();
    parser.leave_prefix_expression();
    CompletedNode { start_event }
}

fn emit_missing_left_binary(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    operator: &str,
) -> CompletedNode {
    let start_event = parser.event_position();
    let (_, right_power, kind) = binary_binding_power(operator)
        .expect("missing-left dispatch uses one known binary operator");
    debug_assert_eq!(kind, SyntaxKind::BinaryExpression);
    let typed_operator = syntax_binary_operator(operator)
        .expect("missing-left dispatch uses the closed binary operator set");
    let operator_range = parser
        .current()
        .expect("missing-left binary retains its operator token")
        .range();
    let owner = parser.start_projected_owner(SyntaxKind::BinaryExpression, role);
    let left = emit_missing_expression(parser, SyntaxRole::LeftOperand);
    let left_range = parser
        .completed_range(left.start_event)
        .expect("missing binary left operand retains one insertion");
    parser.bump();
    parser.bump_trivia_before(end);
    let right = if parser.cursor() < end {
        parse_binding_power(parser, end, right_power, SyntaxRole::RightOperand)
    } else {
        emit_missing_expression(parser, SyntaxRole::RightOperand)
    };
    let right_range = parser
        .completed_range(right.start_event)
        .expect("binary right operand retains one exact source range");
    let right_slot = completed_slot(parser, right);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Binary {
                left: SyntaxExpressionSlot::Missing,
                operator: typed_operator,
                right: right_slot,
            },
            vec![
                PendingExpressionComponent::new(ExpressionComponentRole::LeftOperand, left_range),
                PendingExpressionComponent::new(ExpressionComponentRole::Operator, operator_range),
                PendingExpressionComponent::new(ExpressionComponentRole::RightOperand, right_range),
            ],
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_short_variant(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::ShortVariantExpression, role);
    let marker = parser
        .current()
        .expect("short-variant dispatch retains its leading marker")
        .range();
    parser.bump();
    parser.bump_trivia_before(end);
    let (name, name_range) = if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        ) {
        let token = parser.current().expect("preflighted short-variant name");
        let name = SyntaxName::try_new(parser.text_of(token));
        parser.start(SyntaxKind::NameReference, SyntaxRole::Target);
        parser.bump();
        parser.finish();
        (name, token.range())
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Target);
        parser.finish();
        (
            Err(SyntaxNameIssue::Missing),
            arcweft_source::SourceRange::new(at, at),
        )
    };
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::ShortVariant(name),
            vec![
                PendingExpressionComponent::new(
                    ExpressionComponentRole::ShortVariantMarker,
                    marker,
                ),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::ShortVariantName,
                    name_range,
                ),
            ],
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_path_expression(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::PathExpression, role);
    emit_path(
        parser,
        end,
        SyntaxRole::Target,
        PathSeparatorGrammar::QualifiedOnly,
    );
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::Path, Vec::new()),
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_literal(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) -> CompletedNode {
    let start_event = parser.event_position();
    let token = parser
        .current()
        .expect("literal dispatch retains one token");
    let projection = typed_literal(token, parser.text_of(token));
    let components = projection
        .components()
        .iter()
        .map(|component| {
            PendingExpressionComponent::new(
                ExpressionComponentRole::Literal(match component.part() {
                    LiteralLexemePart::Body => ExpressionLiteralPart::Body,
                    LiteralLexemePart::Prefix => ExpressionLiteralPart::Prefix,
                    LiteralLexemePart::Suffix => ExpressionLiteralPart::Suffix,
                    LiteralLexemePart::Unit => ExpressionLiteralPart::Unit,
                }),
                component.range(),
            )
        })
        .collect();
    let syntax = projection.into_syntax();
    let owner = parser.start_projected_owner(SyntaxKind::LiteralExpression, role);
    parser.bump();
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::Literal(syntax), components),
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_entity_reference(
    parser: &mut DocumentParser<'_, '_>,
    role: SyntaxRole,
) -> (CompletedNode, crate::id_ref::SyntaxIdRefSyntax) {
    let start_event = parser.event_position();
    let token = parser
        .current()
        .expect("entity-reference dispatch retains one token");
    let projection = typed_entity_reference(token, parser.text_of(token));
    let components = projection
        .components()
        .iter()
        .map(|component| {
            PendingExpressionComponent::new(
                ExpressionComponentRole::EntityReference(component.part()),
                component.range(),
            )
        })
        .collect();
    let syntax = projection.into_syntax();
    let retained = syntax.clone();
    let owner = parser.start_projected_owner(SyntaxKind::EntityReferenceExpression, role);
    parser.bump();
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::EntityReference(syntax), components),
    );
    parser.finish();
    (CompletedNode { start_event }, retained)
}

fn emit_lifetime_path(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) -> CompletedNode {
    let start_event = parser.event_position();
    let token = parser
        .current()
        .expect("lifetime-path dispatch retains one token");
    let projection = typed_lifetime_registry_path(token, parser.text_of(token));
    let components = projection.components().to_vec();
    let syntax = projection.into_syntax();
    let owner = parser.start_projected_owner(SyntaxKind::LifetimePathExpression, role);
    parser.bump();
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::LifetimePath(syntax), components),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_placeholder(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) -> CompletedNode {
    let start_event = parser.event_position();
    let token = parser
        .current()
        .expect("placeholder dispatch retains one marker");
    let kind = match parser.text_of(token) {
        "_" => SyntaxPlaceholderKind::PartialApplication,
        "^" => SyntaxPlaceholderKind::PipeLeft,
        _ => unreachable!("placeholder dispatch is closed over `_` and `^`"),
    };
    let owner = parser.start_projected_owner(SyntaxKind::PlaceholderExpression, role);
    parser.bump();
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Placeholder(kind),
            vec![PendingExpressionComponent::new(
                ExpressionComponentRole::PlaceholderMarker,
                token.range(),
            )],
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_error(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) -> CompletedNode {
    let start_event = parser.event_position();
    let token = parser
        .current()
        .expect("generic expression recovery retains one authored token");
    let owner = parser.start_projected_owner(SyntaxKind::ErrorExpression, role);
    parser.bump();
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Error,
            vec![PendingExpressionComponent::new(
                ExpressionComponentRole::Recovery,
                token.range(),
            )],
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_callback_block_call(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    let callee_range = parser
        .completed_range(left.start_event)
        .expect("callback Call callee retains one exact source range");
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::CallExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Callee);
    let callback = emit_callback_closure(parser, end);
    let terminal_call_role = match callback.call_terminator {
        SyntaxCallArgumentListTerminator::Closed => ExpressionComponentRole::CallArgumentListClose,
        SyntaxCallArgumentListTerminator::RecoveredMissing => {
            ExpressionComponentRole::CallArgumentListRecoveryEnd
        }
    };
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Call(SyntaxCallProjection::CallbackBlock(
                SyntaxCallbackBlockCallProjection::new(callback.slot, callback.call_terminator),
            )),
            vec![
                PendingExpressionComponent::new(ExpressionComponentRole::CallCallee, callee_range),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgumentListOpen,
                    callback.open,
                ),
                PendingExpressionComponent::new(terminal_call_role, callback.terminal_range),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgument {
                        argument: 0,
                        part: SyntaxCallArgumentPart::Whole,
                    },
                    callback.range,
                ),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgument {
                        argument: 0,
                        part: SyntaxCallArgumentPart::Value,
                    },
                    callback.range,
                ),
            ],
        ),
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

struct EmittedCallbackClosure {
    slot: SyntaxExpressionSlot,
    range: SourceRange,
    open: SourceRange,
    call_terminator: SyntaxCallArgumentListTerminator,
    terminal_range: SourceRange,
}

fn emit_callback_closure(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
) -> EmittedCallbackClosure {
    let callback_start = parser.event_position();
    let closure =
        parser.start_projected_owner(SyntaxKind::ClosureExpression, SyntaxRole::Argument(0));
    let open_index = parser.cursor();
    let close = find_matching_close(parser, open_index + 1, "{")
        .filter(|close| *close < end)
        .unwrap_or(end);
    let open = parser
        .current()
        .expect("callback Call retains its opening brace")
        .range();
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    parser.bump_trivia_before(close);
    let (parameters, mut components, explicit_header) = emit_callback_closure_header(parser, close);
    let (body, _) = expression_slot(parser, close);
    let body_expression = if find_statement_terminator(parser, parser.cursor(), close).is_some() {
        control::emit_unbraced_block_expression(parser, close, SyntaxRole::Body)
    } else {
        emit_expression_node(parser, close, SyntaxRole::Body)
    };
    bump_until(parser, close);
    let body_range = parser
        .completed_range(body_expression.start_event)
        .expect("callback Closure retains one exact body source range");
    let terminal = emit_callback_closure_terminator(parser, close);
    components.extend([
        PendingExpressionComponent::new(ExpressionComponentRole::ClosureOpenDelimiter, open),
        PendingExpressionComponent::new(terminal.role, terminal.range),
        PendingExpressionComponent::new(ExpressionComponentRole::Body, body_range),
    ]);
    parser.set_expression_projection(
        closure,
        PendingExpressionProjection::new(
            ExpressionProjection::Closure(SyntaxClosureProjection::new(
                parameters,
                false,
                body,
                SyntaxClosureSyntax::CallbackBlock {
                    explicit_header,
                    terminator: terminal.closure,
                },
            )),
            components,
        ),
    );
    parser.finish();
    let callback = CompletedNode {
        start_event: callback_start,
    };
    let range = parser
        .completed_range(callback.start_event)
        .expect("callback Closure retains one exact source range");
    EmittedCallbackClosure {
        slot: completed_slot(parser, callback),
        range,
        open,
        call_terminator: terminal.call,
        terminal_range: terminal.range,
    }
}

fn emit_callback_closure_header(
    parser: &mut DocumentParser<'_, '_>,
    close: usize,
) -> (
    Vec<crate::expressions::SyntaxClosureParameterProjection>,
    Vec<PendingExpressionComponent>,
    bool,
) {
    let arrow = find_top_level_boundary(parser, parser.cursor(), close, &["=>", "}"]);
    let explicit = arrow < close
        && parser
            .token_at(arrow)
            .is_some_and(|token| parser.text_of(token) == "=>");
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let (parameters, mut components) = if explicit {
        composite::emit_closure_parameters_until(parser, arrow)
    } else {
        (Vec::new(), Vec::new())
    };
    parser.finish();
    if explicit {
        bump_until(parser, arrow);
        let fat_arrow = parser
            .bump()
            .expect("callback header retains its fat arrow")
            .range();
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::ClosureFatArrow,
            fat_arrow,
        ));
        parser.bump_trivia_before(close);
    }
    (parameters, components, explicit)
}

struct EmittedCallbackTerminator {
    closure: SyntaxClosureTerminator,
    call: SyntaxCallArgumentListTerminator,
    role: ExpressionComponentRole,
    range: SourceRange,
}

fn emit_callback_closure_terminator(
    parser: &mut DocumentParser<'_, '_>,
    close: usize,
) -> EmittedCallbackTerminator {
    if parser.cursor() == close && parser.at("}") {
        let range = parser
            .current()
            .expect("callback Call retains its closing brace")
            .range();
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.expression.missing_callback_close",
        );
        EmittedCallbackTerminator {
            closure: SyntaxClosureTerminator::Closed,
            call: SyntaxCallArgumentListTerminator::Closed,
            role: ExpressionComponentRole::ClosureCloseDelimiter,
            range,
        }
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        EmittedCallbackTerminator {
            closure: SyntaxClosureTerminator::RecoveredMissing,
            call: SyntaxCallArgumentListTerminator::RecoveredMissing,
            role: ExpressionComponentRole::ClosureRecoveryEnd,
            range: SourceRange::new(at, at),
        }
    }
}

fn emit_postfix(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
    operator: &str,
) -> CompletedNode {
    match operator {
        "(" => emit_call(parser, end, left, role),
        "[" => emit_postfix_bracket(parser, end, left, role),
        "." => emit_select(parser, end, left, role),
        _ => left,
    }
}

enum PreparedCallTypeArgument {
    Present(PreparedTypeProjection),
    InvalidPresent {
        start: usize,
        end: usize,
        error: crate::types::TypeParseError,
        range: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PreparedCallTypeArgument {
    fn range(&self) -> SourceRange {
        match self {
            Self::Present(prepared) => {
                let range = prepared.whole();
                SourceRange::new(range.start(), range.end())
            }
            Self::InvalidPresent { range, .. } | Self::Missing { insertion: range } => *range,
        }
    }

    const fn projection(&self) -> SyntaxCallTypeArgumentProjection {
        match self {
            Self::Present(_) => SyntaxCallTypeArgumentProjection::Present,
            Self::InvalidPresent { .. } => SyntaxCallTypeArgumentProjection::InvalidPresent,
            Self::Missing { .. } => SyntaxCallTypeArgumentProjection::Missing,
        }
    }
}

#[derive(Clone, Copy)]
enum PreparedCallTypeTerminator {
    Closed { index: usize },
    RecoveredMissing { call_open: usize },
    InvalidPresent { index: usize },
}

struct PreparedCallTypeApplication {
    start: usize,
    spelling: SyntaxCallTypeApplicationSpelling,
    turbofish_separator: Option<usize>,
    open: usize,
    arguments: Vec<PreparedCallTypeArgument>,
    separators: Vec<usize>,
    trailing_separator: bool,
    terminator: PreparedCallTypeTerminator,
    call_open: usize,
}

#[derive(Clone, Copy)]
struct PreparedCallTypeApplicationHead {
    start: usize,
    spelling: SyntaxCallTypeApplicationSpelling,
    turbofish_separator: Option<usize>,
    open: usize,
}

#[derive(Clone, Copy)]
struct PreparedCallTypeApplicationTail {
    content_end: usize,
    terminator: PreparedCallTypeTerminator,
    call_open: usize,
}

fn prepare_terminal_call_type_application(
    parser: &DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    start: usize,
) -> Option<PreparedCallTypeApplication> {
    let left_kind = parser.completed_kind(left.start_event)?;
    let spelling = match parser.token_at(start).map(|token| parser.text_of(token))? {
        "<" if left_kind == SyntaxKind::SelectExpression => {
            SyntaxCallTypeApplicationSpelling::DirectAngle
        }
        "::" if matches!(
            left_kind,
            SyntaxKind::PathExpression | SyntaxKind::SelectExpression
        ) =>
        {
            SyntaxCallTypeApplicationSpelling::Turbofish
        }
        _ => return None,
    };
    prepare_call_type_application(parser, end, start, spelling)
}

fn prepare_call_type_application(
    parser: &DocumentParser<'_, '_>,
    end: usize,
    start: usize,
    spelling: SyntaxCallTypeApplicationSpelling,
) -> Option<PreparedCallTypeApplication> {
    let head = prepare_call_type_application_head(parser, end, start, spelling)?;
    let tail = prepare_call_type_application_tail(parser, end, head)?;
    Some(prepare_call_type_application_payload(parser, head, tail))
}

fn prepare_call_type_application_head(
    parser: &DocumentParser<'_, '_>,
    end: usize,
    start: usize,
    spelling: SyntaxCallTypeApplicationSpelling,
) -> Option<PreparedCallTypeApplicationHead> {
    let (turbofish_separator, open) = match spelling {
        SyntaxCallTypeApplicationSpelling::DirectAngle => (None, start),
        SyntaxCallTypeApplicationSpelling::Turbofish => {
            if parser
                .token_at(start)
                .is_none_or(|token| parser.text_of(token) != "::")
            {
                return None;
            }
            let open = next_significant_index(parser, start + 1, end)?;
            if parser
                .token_at(open)
                .is_none_or(|token| parser.text_of(token) != "<")
            {
                return None;
            }
            (Some(start), open)
        }
    };
    if parser
        .token_at(open)
        .is_none_or(|token| parser.text_of(token) != "<")
    {
        return None;
    }
    Some(PreparedCallTypeApplicationHead {
        start,
        spelling,
        turbofish_separator,
        open,
    })
}

fn prepare_call_type_application_tail(
    parser: &DocumentParser<'_, '_>,
    end: usize,
    head: PreparedCallTypeApplicationHead,
) -> Option<PreparedCallTypeApplicationTail> {
    let mut angle_depth = 1_usize;
    let mut paren_depth = 0_usize;
    let mut bracket_depth = 0_usize;
    let mut brace_depth = 0_usize;
    let mut missing_close_candidates = Vec::new();
    for index in head.open + 1..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        if is_expression_trivia(token.kind()) {
            continue;
        }
        let text = parser.text_of(token);
        let nested = paren_depth > 0 || bracket_depth > 0 || brace_depth > 0;
        if !nested && angle_depth == 1 {
            if text == "(" {
                missing_close_candidates.push(index);
            }
            if matches!(text, "]" | "}")
                && next_significant_index(parser, index + 1, end).is_some_and(|next| {
                    parser
                        .token_at(next)
                        .is_some_and(|token| parser.text_of(token) == "(")
                })
            {
                let call_open = next_significant_index(parser, index + 1, end)?;
                return Some(PreparedCallTypeApplicationTail {
                    content_end: index,
                    terminator: PreparedCallTypeTerminator::InvalidPresent { index },
                    call_open,
                });
            }
        }
        match text {
            "<" if !nested => angle_depth = angle_depth.saturating_add(1),
            ">" if !nested => {
                angle_depth = angle_depth.saturating_sub(1);
                if angle_depth == 0 {
                    let call_open = next_significant_index(parser, index + 1, end)?;
                    if parser
                        .token_at(call_open)
                        .is_none_or(|token| parser.text_of(token) != "(")
                    {
                        return None;
                    }
                    return Some(PreparedCallTypeApplicationTail {
                        content_end: index,
                        terminator: PreparedCallTypeTerminator::Closed { index },
                        call_open,
                    });
                }
            }
            "(" => paren_depth = paren_depth.saturating_add(1),
            ")" => paren_depth = paren_depth.saturating_sub(1),
            "[" => bracket_depth = bracket_depth.saturating_add(1),
            "]" => bracket_depth = bracket_depth.saturating_sub(1),
            "{" => brace_depth = brace_depth.saturating_add(1),
            "}" => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }

    let call_open = missing_close_candidates.into_iter().next_back()?;
    Some(PreparedCallTypeApplicationTail {
        content_end: call_open,
        terminator: PreparedCallTypeTerminator::RecoveredMissing { call_open },
        call_open,
    })
}

fn prepare_call_type_application_payload(
    parser: &DocumentParser<'_, '_>,
    head: PreparedCallTypeApplicationHead,
    tail: PreparedCallTypeApplicationTail,
) -> PreparedCallTypeApplication {
    let separators = top_level_type_argument_separators(parser, head.open + 1, tail.content_end);
    let trailing_separator = separators.last().is_some_and(|separator| {
        next_significant_index(parser, separator + 1, tail.content_end).is_none()
    });
    let mut bounds = Vec::with_capacity(separators.len() + 2);
    bounds.push(head.open + 1);
    bounds.extend(separators.iter().map(|separator| separator + 1));
    bounds.push(tail.content_end);
    let slot_count = if trailing_separator {
        bounds.len().saturating_sub(2)
    } else {
        bounds.len().saturating_sub(1)
    };
    let mut arguments = Vec::with_capacity(slot_count.max(1));
    for slot in 0..slot_count {
        let raw_start = bounds[slot];
        let raw_end = if slot < separators.len() {
            separators[slot]
        } else {
            tail.content_end
        };
        arguments.push(prepare_call_type_argument(parser, raw_start, raw_end));
    }
    if arguments.is_empty() {
        let at = parser
            .token_at(head.open)
            .expect("prepared type application retains its opening angle")
            .range()
            .end();
        arguments.push(PreparedCallTypeArgument::Missing {
            insertion: SourceRange::new(at, at),
        });
    }
    PreparedCallTypeApplication {
        start: head.start,
        spelling: head.spelling,
        turbofish_separator: head.turbofish_separator,
        open: head.open,
        arguments,
        separators,
        trailing_separator,
        terminator: tail.terminator,
        call_open: tail.call_open,
    }
}

fn prepare_call_type_argument(
    parser: &DocumentParser<'_, '_>,
    raw_start: usize,
    raw_end: usize,
) -> PreparedCallTypeArgument {
    let start = next_significant_index(parser, raw_start, raw_end).unwrap_or(raw_end);
    let end = trimmed_end(parser, start, raw_end);
    if start >= end {
        let at = parser
            .offset_at_token_boundary(start)
            .unwrap_or_else(|| parser.source().len());
        return PreparedCallTypeArgument::Missing {
            insertion: SourceRange::new(at, at),
        };
    }
    match prepare_type(parser, start, end) {
        Ok(prepared) => PreparedCallTypeArgument::Present(prepared),
        Err(error) => {
            let start_offset = parser
                .offset_at_token_boundary(start)
                .expect("type argument starts at a token boundary");
            let end_offset = parser
                .offset_at_token_boundary(end)
                .expect("type argument ends at a token boundary");
            PreparedCallTypeArgument::InvalidPresent {
                start,
                end,
                error,
                range: SourceRange::new(start_offset, end_offset),
            }
        }
    }
}

fn top_level_type_argument_separators(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Vec<usize> {
    let mut angle = 0_usize;
    let mut paren = 0_usize;
    let mut bracket = 0_usize;
    let mut brace = 0_usize;
    let mut separators = Vec::new();
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        if is_expression_trivia(token.kind()) {
            continue;
        }
        let text = parser.text_of(token);
        if text == "," && angle == 0 && paren == 0 && bracket == 0 && brace == 0 {
            separators.push(index);
            continue;
        }
        match text {
            "<" => angle = angle.saturating_add(1),
            ">" => angle = angle.saturating_sub(1),
            "(" => paren = paren.saturating_add(1),
            ")" => paren = paren.saturating_sub(1),
            "[" => bracket = bracket.saturating_add(1),
            "]" => bracket = bracket.saturating_sub(1),
            "{" => brace = brace.saturating_add(1),
            "}" => brace = brace.saturating_sub(1),
            _ => {}
        }
    }
    separators
}

fn next_significant_index(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    (start..end).find(|index| {
        parser
            .token_at(*index)
            .is_some_and(|token| !is_expression_trivia(token.kind()))
    })
}

fn is_expression_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

fn emit_typed_call(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
    application: PreparedCallTypeApplication,
) -> CompletedNode {
    let callee_range = parser
        .completed_range(left.start_event)
        .expect("completed typed Call callee retains one exact source range");
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::CallExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Callee);
    let (type_application, mut components) = emit_call_type_application(parser, application);
    bump_until(parser, type_application.call_open);
    let tail = emit_parenthesized_call_tail(parser, end);
    components.insert(
        0,
        PendingExpressionComponent::new(ExpressionComponentRole::CallCallee, callee_range),
    );
    components.extend(tail.components);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(
                SyntaxParenthesizedCallProjection::ordinary(
                    Some(type_application.projection),
                    tail.arguments,
                    tail.terminator,
                ),
            )),
            components,
        ),
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

struct EmittedCallTypeApplication {
    projection: SyntaxCallTypeApplicationProjection,
    call_open: usize,
}

fn emit_call_type_application(
    parser: &mut DocumentParser<'_, '_>,
    prepared: PreparedCallTypeApplication,
) -> (EmittedCallTypeApplication, Vec<PendingExpressionComponent>) {
    let ranges = call_type_application_ranges(parser, &prepared);
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::CallTypeApplication(SyntaxCallTypeApplicationComponentRole::Whole),
        ranges.whole,
    )];
    let projections = prepared
        .arguments
        .iter()
        .map(PreparedCallTypeArgument::projection)
        .collect::<Vec<_>>();
    emit_call_type_application_head(parser, &prepared, ranges.open, &mut components);
    emit_call_type_arguments(
        parser,
        prepared.arguments,
        &prepared.separators,
        projections.len(),
        &mut components,
    );
    emit_call_type_trailing_separator(
        parser,
        prepared.trailing_separator,
        &prepared.separators,
        &mut components,
    );
    emit_empty_call_type_insertion(
        &projections,
        &prepared.separators,
        ranges.open,
        &mut components,
    );
    let terminator = emit_call_type_terminator(
        parser,
        prepared.terminator,
        ranges.terminator,
        &mut components,
    );
    (
        EmittedCallTypeApplication {
            projection: SyntaxCallTypeApplicationProjection::new(
                prepared.spelling,
                projections,
                terminator,
            ),
            call_open: prepared.call_open,
        },
        components,
    )
}

struct CallTypeApplicationRanges {
    open: SourceRange,
    terminator: SourceRange,
    whole: SourceRange,
}

fn call_type_application_ranges(
    parser: &DocumentParser<'_, '_>,
    prepared: &PreparedCallTypeApplication,
) -> CallTypeApplicationRanges {
    let open = parser
        .token_at(prepared.open)
        .expect("prepared type application retains its opening angle")
        .range();
    let terminator = match prepared.terminator {
        PreparedCallTypeTerminator::Closed { index }
        | PreparedCallTypeTerminator::InvalidPresent { index } => parser
            .token_at(index)
            .expect("authored type application terminator remains attached")
            .range(),
        PreparedCallTypeTerminator::RecoveredMissing { call_open } => {
            let at = parser
                .offset_at_token_boundary(call_open)
                .expect("recovered type application ends at the Call opening boundary");
            SourceRange::new(at, at)
        }
    };
    let whole_start = parser
        .token_at(prepared.start)
        .expect("prepared type application retains its first token")
        .range()
        .start();
    CallTypeApplicationRanges {
        open,
        terminator,
        whole: SourceRange::new(whole_start, terminator.end()),
    }
}

fn emit_call_type_application_head(
    parser: &mut DocumentParser<'_, '_>,
    prepared: &PreparedCallTypeApplication,
    open_range: SourceRange,
    components: &mut Vec<PendingExpressionComponent>,
) {
    if let Some(separator) = prepared.turbofish_separator {
        bump_until(parser, separator);
        let range = parser
            .current()
            .expect("prepared turbofish retains its separator")
            .range();
        parser.bump();
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::CallTypeApplication(
                SyntaxCallTypeApplicationComponentRole::TurbofishSeparator,
            ),
            range,
        ));
    }
    bump_until(parser, prepared.open);
    emit_open_delimiter(parser, SyntaxKind::OpenAngleNode, "<");
    components.push(PendingExpressionComponent::new(
        ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::OpenAngle,
        ),
        open_range,
    ));
}

fn emit_call_type_arguments(
    parser: &mut DocumentParser<'_, '_>,
    arguments: Vec<PreparedCallTypeArgument>,
    separators: &[usize],
    argument_count: usize,
    components: &mut Vec<PendingExpressionComponent>,
) {
    for (ordinal, argument) in arguments.into_iter().enumerate() {
        let ordinal = u16::try_from(ordinal)
            .expect("document grammar budget bounds Call type argument ordinals");
        let range = argument.range();
        emit_call_type_argument(parser, argument);
        for part in [
            SyntaxCallTypeArgumentPart::Whole,
            SyntaxCallTypeArgumentPart::Type,
        ] {
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::CallTypeApplication(
                    SyntaxCallTypeApplicationComponentRole::Argument {
                        argument: ordinal,
                        part,
                    },
                ),
                range,
            ));
        }
        if usize::from(ordinal) + 1 < argument_count {
            emit_call_type_separator(parser, separators, ordinal, components);
        }
    }
}

fn emit_call_type_argument(
    parser: &mut DocumentParser<'_, '_>,
    argument: PreparedCallTypeArgument,
) {
    match argument {
        PreparedCallTypeArgument::Present(prepared) => {
            bump_until(parser, prepared.start());
            let _ = emit_prepared_type(parser, SyntaxRole::Type, prepared);
        }
        PreparedCallTypeArgument::InvalidPresent {
            start, end, error, ..
        } => {
            bump_until(parser, start);
            let _ = emit_recovered_type(parser, SyntaxRole::Type, start, end, &error);
        }
        PreparedCallTypeArgument::Missing { .. } => {}
    }
}

fn emit_call_type_separator(
    parser: &mut DocumentParser<'_, '_>,
    separators: &[usize],
    ordinal: u16,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let separator = separators[usize::from(ordinal)];
    bump_until(parser, separator);
    let range = parser
        .bump()
        .expect("prepared type argument separator remains attached")
        .range();
    components.push(PendingExpressionComponent::new(
        ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::Separator {
                following: ordinal + 1,
            },
        ),
        range,
    ));
}

fn emit_call_type_trailing_separator(
    parser: &mut DocumentParser<'_, '_>,
    trailing_separator: bool,
    separators: &[usize],
    components: &mut Vec<PendingExpressionComponent>,
) {
    if !trailing_separator {
        return;
    }
    let separator = *separators
        .last()
        .expect("trailing separator state retains one comma");
    bump_until(parser, separator);
    let range = parser
        .bump()
        .expect("prepared trailing type separator remains attached")
        .range();
    components.push(PendingExpressionComponent::new(
        ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::TrailingSeparator,
        ),
        range,
    ));
}

fn emit_empty_call_type_insertion(
    projections: &[SyntaxCallTypeArgumentProjection],
    separators: &[usize],
    open_range: SourceRange,
    components: &mut Vec<PendingExpressionComponent>,
) {
    if projections.len() == 1
        && matches!(projections[0], SyntaxCallTypeArgumentProjection::Missing)
        && separators.is_empty()
    {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::CallTypeApplication(
                SyntaxCallTypeApplicationComponentRole::EmptyInsertion,
            ),
            SourceRange::new(open_range.end(), open_range.end()),
        ));
    }
}

fn emit_call_type_terminator(
    parser: &mut DocumentParser<'_, '_>,
    terminator: PreparedCallTypeTerminator,
    terminator_range: SourceRange,
    components: &mut Vec<PendingExpressionComponent>,
) -> SyntaxCallTypeApplicationTerminator {
    match terminator {
        PreparedCallTypeTerminator::Closed { index } => {
            bump_until(parser, index);
            emit_close_delimiter(
                parser,
                SyntaxKind::CloseAngleNode,
                ">",
                "syntax.expression.missing_call_type_close",
            );
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::CallTypeApplication(
                    SyntaxCallTypeApplicationComponentRole::CloseAngle,
                ),
                terminator_range,
            ));
            SyntaxCallTypeApplicationTerminator::Closed
        }
        PreparedCallTypeTerminator::RecoveredMissing { call_open } => {
            bump_until(parser, call_open);
            emit_close_delimiter(
                parser,
                SyntaxKind::CloseAngleNode,
                ">",
                "syntax.expression.missing_call_type_close",
            );
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::CallTypeApplication(
                    SyntaxCallTypeApplicationComponentRole::RecoveryEnd,
                ),
                terminator_range,
            ));
            SyntaxCallTypeApplicationTerminator::RecoveredMissing
        }
        PreparedCallTypeTerminator::InvalidPresent { index } => {
            bump_until(parser, index);
            parser.start(SyntaxKind::CloseAngleNode, SyntaxRole::CloseDelimiter);
            parser.bump();
            parser.finish();
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::CallTypeApplication(
                    SyntaxCallTypeApplicationComponentRole::CloseAngle,
                ),
                terminator_range,
            ));
            SyntaxCallTypeApplicationTerminator::InvalidPresent
        }
    }
}

fn emit_call(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    if parser.budget_failed() {
        bump_until(parser, end);
        return left;
    }
    let callee_range = parser
        .completed_range(left.start_event)
        .expect("completed Call callee retains one exact source range");
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::CallExpression, role);
    if parser.budget_failed() {
        bump_until(parser, end);
        return left;
    }
    parser.set_start_role(left.start_event + 1, SyntaxRole::Callee);
    let tail = emit_parenthesized_call_tail(parser, end);
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::CallCallee,
        callee_range,
    )];
    components.extend(tail.components);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(
                SyntaxParenthesizedCallProjection::ordinary(None, tail.arguments, tail.terminator),
            )),
            components,
        ),
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

struct PreparedAssociatedCall {
    receiver: PreparedAssociatedReceiver,
    separator: PreparedAssociatedSeparator,
    member: PreparedAssociatedMember,
    type_application: Option<PreparedCallTypeApplication>,
    call_open: usize,
}

enum PreparedAssociatedReceiver {
    Present(PreparedTypeProjection),
    InvalidPresent {
        start: usize,
        end: usize,
        error: crate::types::TypeParseError,
        range: SourceRange,
    },
}

impl PreparedAssociatedReceiver {
    fn range(&self) -> SourceRange {
        match self {
            Self::Present(prepared) => {
                let range = prepared.whole();
                SourceRange::new(range.start(), range.end())
            }
            Self::InvalidPresent { range, .. } => *range,
        }
    }

    const fn projection() -> SyntaxAssociatedReceiver {
        SyntaxAssociatedReceiver::Present
    }
}

#[derive(Clone, Copy)]
struct PreparedAssociatedSeparator {
    index: usize,
    syntax: SyntaxAssociatedCallSyntax,
}

impl PreparedAssociatedSeparator {
    const fn projection(self) -> SyntaxAssociatedSeparator {
        SyntaxAssociatedSeparator::Present(self.syntax)
    }
}

enum PreparedAssociatedMember {
    Present {
        index: usize,
        range: SourceRange,
        name: Result<SyntaxName, SyntaxNameIssue>,
    },
    Missing {
        insertion: SourceRange,
    },
}

fn emit_associated_call(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> Option<CompletedNode> {
    let prepared = prepare_associated_call(parser, end)?;
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::CallExpression, role);
    let receiver_range = prepared.receiver.range();
    let receiver_projection = PreparedAssociatedReceiver::projection();
    let separator_projection = prepared.separator.projection();
    let unresolved_dot = matches!(&prepared.receiver, PreparedAssociatedReceiver::Present(_))
        && prepared.separator.syntax == SyntaxAssociatedCallSyntax::DotFallback
        && matches!(
            &prepared.member,
            PreparedAssociatedMember::Present { name: Ok(_), .. }
        );

    if unresolved_dot {
        let value = parser.start_projected_owner(SyntaxKind::PathExpression, SyntaxRole::Callee);
        let PreparedAssociatedReceiver::Present(receiver) = prepared.receiver else {
            unreachable!("unresolved dot retains one valid nominal receiver")
        };
        let _ = emit_prepared_type(parser, SyntaxRole::Type, receiver);
        parser.set_expression_projection(
            value,
            PendingExpressionProjection::new(ExpressionProjection::Path, Vec::new()),
        );
        parser.finish();
    } else {
        emit_associated_receiver(parser, prepared.receiver);
    }

    let separator_range = emit_associated_separator(parser, prepared.separator);
    let (member, member_range) = emit_associated_member(parser, prepared.member);
    let (type_application, type_components) = if let Some(application) = prepared.type_application {
        bump_until(parser, application.start);
        let (application, components) = emit_call_type_application(parser, application);
        (Some(application.projection), components)
    } else {
        (None, Vec::new())
    };
    bump_until(parser, prepared.call_open);

    let tail = emit_parenthesized_call_tail(parser, end);
    let mut components = vec![
        PendingExpressionComponent::new(
            ExpressionComponentRole::CallAssociatedReceiver,
            receiver_range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::CallAssociatedSeparator,
            separator_range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::CallAssociatedMember,
            member_range,
        ),
    ];
    components.extend(type_components);
    components.extend(tail.components);
    let projection = if unresolved_dot {
        SyntaxParenthesizedCallProjection::unresolved_dot(
            separator_projection,
            member,
            type_application,
            tail.arguments,
            tail.terminator,
        )
    } else {
        SyntaxParenthesizedCallProjection::associated(
            receiver_projection,
            separator_projection,
            member,
            type_application,
            tail.arguments,
            tail.terminator,
        )
    };
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(projection)),
            components,
        ),
    );
    parser.finish();
    Some(CompletedNode { start_event })
}

fn emit_associated_receiver(
    parser: &mut DocumentParser<'_, '_>,
    receiver: PreparedAssociatedReceiver,
) {
    match receiver {
        PreparedAssociatedReceiver::Present(receiver) => {
            let _ = emit_prepared_type(parser, SyntaxRole::Type, receiver);
        }
        PreparedAssociatedReceiver::InvalidPresent {
            start, end, error, ..
        } => {
            bump_until(parser, start);
            let _ = emit_recovered_type(parser, SyntaxRole::Type, start, end, &error);
        }
    }
}

fn emit_associated_separator(
    parser: &mut DocumentParser<'_, '_>,
    separator: PreparedAssociatedSeparator,
) -> SourceRange {
    bump_until(parser, separator.index);
    parser
        .bump()
        .expect("associated Call retains its separator token")
        .range()
}

fn emit_associated_member(
    parser: &mut DocumentParser<'_, '_>,
    member: PreparedAssociatedMember,
) -> (Result<SyntaxName, SyntaxNameIssue>, SourceRange) {
    match member {
        PreparedAssociatedMember::Present { index, range, name } => {
            bump_until(parser, index);
            parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
            parser.bump();
            parser.finish();
            (name, range)
        }
        PreparedAssociatedMember::Missing { insertion } => {
            (Err(SyntaxNameIssue::Missing), insertion)
        }
    }
}

fn prepare_associated_call(
    parser: &DocumentParser<'_, '_>,
    end: usize,
) -> Option<PreparedAssociatedCall> {
    let start = parser.cursor();
    let call_open = find_top_level_boundary(parser, start, end, &["("]);
    for call_open in [call_open] {
        if parser
            .token_at(call_open)
            .is_none_or(|token| parser.text_of(token) != "(")
        {
            continue;
        }
        let type_application = terminal_type_application_before_call(parser, end, start, call_open);
        let terminal_start = type_application
            .as_ref()
            .map_or(call_open, |application| application.start);
        let Some(tail) = previous_significant(parser, start, terminal_start) else {
            continue;
        };
        let tail_text = parser.token_at(tail).map(|token| parser.text_of(token))?;
        let (member, separator_start, separator_syntax) = if matches!(tail_text, "." | "::") {
            let insertion = parser
                .offset_at_token_boundary(terminal_start)
                .map(|at| SourceRange::new(at, at))?;
            let syntax = if tail_text == "::" {
                SyntaxAssociatedCallSyntax::ExplicitDoubleColon
            } else {
                SyntaxAssociatedCallSyntax::DotFallback
            };
            (
                PreparedAssociatedMember::Missing { insertion },
                tail,
                syntax,
            )
        } else {
            let member_token = parser.token_at(tail)?;
            let member = PreparedAssociatedMember::Present {
                index: tail,
                range: member_token.range(),
                name: SyntaxName::try_new(parser.text_of(member_token)),
            };
            let separator = previous_significant(parser, start, tail);
            match separator.and_then(|index| {
                parser
                    .token_at(index)
                    .map(|token| (index, parser.text_of(token), token.range()))
            }) {
                Some((index, "::", _)) => (
                    member,
                    index,
                    SyntaxAssociatedCallSyntax::ExplicitDoubleColon,
                ),
                Some((index, ".", _)) => (member, index, SyntaxAssociatedCallSyntax::DotFallback),
                _ => continue,
            }
        };
        let receiver_end = trimmed_end(parser, start, separator_start);
        if receiver_end <= start {
            continue;
        }
        let receiver = match prepare_type(parser, start, receiver_end) {
            Ok(receiver) if receiver.authored().value().nominal_path().is_some() => {
                PreparedAssociatedReceiver::Present(receiver)
            }
            Err(error) if separator_syntax == SyntaxAssociatedCallSyntax::ExplicitDoubleColon => {
                let range = SourceRange::new(
                    parser.offset_at_token_boundary(start)?,
                    parser.offset_at_token_boundary(receiver_end)?,
                );
                PreparedAssociatedReceiver::InvalidPresent {
                    start,
                    end: receiver_end,
                    error,
                    range,
                }
            }
            Ok(_) | Err(_) => continue,
        };
        if separator_syntax == SyntaxAssociatedCallSyntax::ExplicitDoubleColon
            && matches!(
                &receiver,
                PreparedAssociatedReceiver::Present(receiver)
                    if !matches!(receiver.authored().value(), TypeRef::Generic { .. })
            )
        {
            continue;
        }
        let separator = PreparedAssociatedSeparator {
            index: separator_start,
            syntax: separator_syntax,
        };
        return Some(PreparedAssociatedCall {
            receiver,
            separator,
            member,
            type_application,
            call_open,
        });
    }
    None
}

fn terminal_type_application_before_call(
    parser: &DocumentParser<'_, '_>,
    end: usize,
    start: usize,
    call_open: usize,
) -> Option<PreparedCallTypeApplication> {
    let mut selected = None;
    for index in start..call_open {
        let spelling = match parser.token_at(index).map(|token| parser.text_of(token)) {
            Some("<")
                if previous_significant(parser, start, index).is_none_or(|previous| {
                    parser
                        .token_at(previous)
                        .is_none_or(|token| parser.text_of(token) != "::")
                }) =>
            {
                SyntaxCallTypeApplicationSpelling::DirectAngle
            }
            Some("::") => SyntaxCallTypeApplicationSpelling::Turbofish,
            _ => continue,
        };
        if let Some(application) = prepare_call_type_application(parser, end, index, spelling)
            && application.call_open == call_open
        {
            selected = Some(application);
        }
    }
    selected
}

fn previous_significant(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    before: usize,
) -> Option<usize> {
    (start..before).rev().find(|index| {
        parser.token_at(*index).is_some_and(|token| {
            !matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            )
        })
    })
}

fn emit_select(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    let target_range = parser
        .completed_range(left.start_event)
        .expect("completed Select target retains one exact source range");
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::SelectExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Target);
    parser.bump();
    while matches!(
        parser.current_kind(),
        Some(SyntaxKind::WhitespaceToken | SyntaxKind::CommentToken)
    ) {
        parser.bump();
    }
    let (member, member_range) = if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        ) {
        let token = parser
            .current()
            .expect("Select member dispatch retains one name token");
        let range = token.range();
        let name = SyntaxName::try_new(parser.text_of(token))
            .expect("Select grammar admits only parser-validated names");
        parser.start(SyntaxKind::NameReference, SyntaxRole::Field(0));
        parser.bump();
        parser.finish();
        (SyntaxSelectedMember::Name(name), range)
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Field(0));
        parser.finish();
        (SyntaxSelectedMember::Missing, SourceRange::new(at, at))
    };
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Select(member),
            vec![
                PendingExpressionComponent::new(ExpressionComponentRole::Target, target_range),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::SelectedMember,
                    member_range,
                ),
            ],
        ),
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

const fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NumberToken
            | SyntaxKind::StringToken
            | SyntaxKind::RawStringToken
            | SyntaxKind::CharacterToken
            | SyntaxKind::UnterminatedStringToken
    )
}

#[cfg(test)]
mod tests;
