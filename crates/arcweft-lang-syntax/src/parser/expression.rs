//! Private Pratt expression grammar over the shared document cursor.

mod composite;
mod control;
mod operators;
mod postfix_bracket;

pub(in crate::parser) use composite::expression_slot;

use self::operators::{binary_binding_power, is_postfix_operator, syntax_binary_operator};
use self::postfix_bracket::emit_postfix_bracket;

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
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
    SyntaxAwaitPropagation, SyntaxBorrowKind, SyntaxCallArgumentListTerminator,
    SyntaxCallArgumentPart, SyntaxCallArgumentProjection, SyntaxCallProjection,
    SyntaxCallTypeApplicationComponentRole, SyntaxCallTypeApplicationProjection,
    SyntaxCallTypeApplicationSpelling, SyntaxCallTypeApplicationTerminator,
    SyntaxCallTypeArgumentPart, SyntaxCallTypeArgumentProjection,
    SyntaxCallbackBlockCallProjection, SyntaxClosureProjection, SyntaxClosureSyntax,
    SyntaxClosureTerminator, SyntaxExpressionSlot, SyntaxParenthesizedCallProjection,
    SyntaxPlaceholderKind, SyntaxRequiredTokenState, SyntaxSelectedMember, SyntaxTryForm,
    SyntaxUnaryOperator,
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

pub(super) fn emit_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let _ = emit_expression_node(parser, end, role);
}

pub(super) fn emit_expression_node(
    parser: &mut ShadowDocumentParser<'_, '_>,
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

fn emit_missing_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::MissingExpression, role);
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn completed_slot(
    parser: &ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    composite::emit_owner_named_block(parser, end, role);
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
            "." | "::" => {}
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

        let left_range = parser
            .completed_range(left.start_event)
            .expect("completed left expression retains one exact source range");
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
                operator_index,
                operator_range,
                right_power,
                operator == "..=",
            );
            continue;
        }
        bump_until(parser, operator_index);
        let owner = if matches!(
            kind,
            SyntaxKind::PipeExpression | SyntaxKind::BinaryExpression
        ) {
            parser.insert_projected_start(left.start_event, kind, role)
        } else {
            parser.insert_start(left.start_event, kind, role);
            None
        };
        parser.set_start_role(left.start_event + 1, SyntaxRole::LeftOperand);
        parser.bump();
        parser.bump_trivia();
        let right = if parser.cursor() < end {
            parse_binding_power(parser, end, right_power, SyntaxRole::RightOperand)
        } else {
            emit_missing_expression(parser, SyntaxRole::RightOperand)
        };
        if matches!(
            kind,
            SyntaxKind::PipeExpression | SyntaxKind::BinaryExpression
        ) {
            let right_range = parser
                .completed_range(right.start_event)
                .expect("completed right expression retains one exact source range");
            let right_slot = completed_slot(parser, right);
            let projection = if kind == SyntaxKind::PipeExpression {
                ExpressionProjection::Pipe([SyntaxExpressionSlot::Authored, right_slot])
            } else {
                ExpressionProjection::Binary {
                    left: SyntaxExpressionSlot::Authored,
                    operator: syntax_binary_operator(operator)
                        .expect("binary binding-power dispatch uses the closed operator set"),
                    right: right_slot,
                }
            };
            parser.set_expression_projection(
                owner,
                PendingExpressionProjection::new(
                    projection,
                    vec![
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::LeftOperand,
                            left_range,
                        ),
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::Operator,
                            operator_range,
                        ),
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::RightOperand,
                            right_range,
                        ),
                    ],
                ),
            );
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

    if text == "choice" {
        return super::statement::choice::emit_choice_expression(parser, end, role);
    }

    if matches!(
        token.kind(),
        SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
    ) && let Some(call) = emit_associated_call(parser, end, role)
    {
        return call;
    }

    match text {
        "&" => emit_prefix_operand(parser, end, SyntaxKind::BorrowExpression, role, true),
        "*" => emit_prefix_operand(parser, end, SyntaxKind::DereferenceExpression, role, false),
        "!" | "-" => emit_prefix_operand(parser, end, SyntaxKind::UnaryExpression, role, false),
        ".." | "..=" => emit_prefix_range(parser, end, role, text == "..="),
        "try" if propagating_await_spelling(parser, end) == Some(PropagatingAwait::TryAwait) => {
            emit_propagating_await(parser, end, role, PropagatingAwait::TryAwait)
        }
        "try" => emit_prefix_operand(parser, end, SyntaxKind::TryExpression, role, false),
        "await"
            if propagating_await_spelling(parser, end) == Some(PropagatingAwait::AwaitQuestion) =>
        {
            emit_propagating_await(parser, end, role, PropagatingAwait::AwaitQuestion)
        }
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser.bump_trivia();
    let mut borrow_kind = SyntaxBorrowKind::Shared;
    if accepts_mutability && parser.at("mut") {
        let mutable = parser
            .current()
            .expect("borrow mutability dispatch retains its token")
            .range();
        operator_range = SourceRange::new(operator_range.start(), mutable.end());
        borrow_kind = SyntaxBorrowKind::Mutable;
        parser.bump();
        parser.bump_trivia();
    }
    let operand = if parser.cursor() < end {
        parse_binding_power(parser, end, 90, SyntaxRole::Operand)
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
    let projection = match kind {
        SyntaxKind::TryExpression => ExpressionProjection::Try {
            operand: operand_slot,
            form: SyntaxTryForm::PrefixTry,
        },
        SyntaxKind::AwaitExpression => ExpressionProjection::Await {
            operand: operand_slot,
            propagation: SyntaxAwaitPropagation::PreserveResult,
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
        PendingExpressionProjection::new(
            projection,
            vec![
                PendingExpressionComponent::new(ExpressionComponentRole::Operator, operator_range),
                PendingExpressionComponent::new(ExpressionComponentRole::Operand, operand_range),
            ],
        ),
    );
    parser.finish();
    parser.leave_prefix_expression();
    CompletedNode { start_event }
}

fn emit_infix_range(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
    operator_index: usize,
    operator_range: SourceRange,
    right_power: u8,
    inclusive: bool,
) -> CompletedNode {
    let left_range = parser
        .completed_range(left.start_event)
        .expect("completed range start retains one exact source range");
    bump_until(parser, operator_index);
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::RangeExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::LeftOperand);
    parser.bump();
    parser.bump_trivia();
    let right = (parser.cursor() < end)
        .then(|| parse_binding_power(parser, end, right_power, SyntaxRole::RightOperand));
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::RangeStart,
        left_range,
    )];
    if inclusive {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RangeInclusiveMarker,
            operator_range,
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
                inclusive,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser.bump_trivia();
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser.bump_trivia();
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PropagatingAwait {
    TryAwait,
    AwaitQuestion,
}

fn propagating_await_spelling(
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
) -> Option<PropagatingAwait> {
    match parser.current_text()? {
        "try" => super::shadow_recovery::first_significant(parser, parser.cursor() + 1, end)
            .filter(|index| super::shadow_recovery::token_text(parser, *index) == Some("await"))
            .map(|_| PropagatingAwait::TryAwait),
        "await" => {
            let question = parser.token_at(parser.cursor() + 1)?;
            (parser.text_of(question) == "?"
                && parser.current()?.range().end() == question.range().start())
            .then_some(PropagatingAwait::AwaitQuestion)
        }
        _ => None,
    }
}

fn emit_propagating_await(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    spelling: PropagatingAwait,
) -> CompletedNode {
    let start_event = parser.event_position();
    if !parser.enter_prefix_expression() {
        bump_until(parser, end);
        return CompletedNode { start_event };
    }
    let owner = parser.start_projected_owner(SyntaxKind::AwaitExpression, role);
    let operator_start = parser
        .bump()
        .expect("propagating await retains its first operator token")
        .range()
        .start();
    parser.bump_trivia();
    match spelling {
        PropagatingAwait::TryAwait => debug_assert!(parser.at("await")),
        PropagatingAwait::AwaitQuestion => debug_assert!(parser.at("?")),
    }
    let operator_end = parser
        .bump()
        .expect("propagating await retains its second operator token")
        .range()
        .end();
    parser.bump_trivia();
    let operand = if parser.cursor() < end {
        parse_binding_power(parser, end, 90, SyntaxRole::Operand)
    } else {
        emit_missing_expression(parser, SyntaxRole::Operand)
    };
    if parser.budget_failed() {
        parser.leave_prefix_expression();
        return CompletedNode { start_event };
    }
    let operand_range = parser
        .completed_range(operand.start_event)
        .expect("completed await operand retains one exact source range");
    let operand_slot = completed_slot(parser, operand);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Await {
                operand: operand_slot,
                propagation: SyntaxAwaitPropagation::PropagateError,
            },
            vec![
                PendingExpressionComponent::new(
                    ExpressionComponentRole::Operator,
                    arcweft_source::SourceRange::new(operator_start, operator_end),
                ),
                PendingExpressionComponent::new(ExpressionComponentRole::Operand, operand_range),
            ],
        ),
    );
    parser.finish();
    parser.leave_prefix_expression();
    CompletedNode { start_event }
}

fn emit_short_variant(
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser.bump_trivia();
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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

pub(super) fn emit_literal(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
) -> CompletedNode {
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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

fn emit_lifetime_path(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
) -> CompletedNode {
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

fn emit_placeholder(parser: &mut ShadowDocumentParser<'_, '_>, role: SyntaxRole) -> CompletedNode {
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

fn emit_error(parser: &mut ShadowDocumentParser<'_, '_>, role: SyntaxRole) -> CompletedNode {
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
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    let callee_range = parser
        .completed_range(left.start_event)
        .expect("callback Call callee retains one exact source range");
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::CallExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Callee);

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
    parser.bump_trivia();

    let arrow = find_top_level_boundary(parser, parser.cursor(), &["=>", "}"]).min(close);
    let explicit_header = arrow < close
        && parser
            .token_at(arrow)
            .is_some_and(|token| parser.text_of(token) == "=>");
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let (parameters, mut closure_components) = if explicit_header {
        composite::emit_closure_parameters_until(parser, arrow)
    } else {
        (Vec::new(), Vec::new())
    };
    parser.finish();
    if explicit_header {
        bump_until(parser, arrow);
        let fat_arrow = parser
            .bump()
            .expect("callback header retains its fat arrow")
            .range();
        closure_components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::ClosureFatArrow,
            fat_arrow,
        ));
        parser.bump_trivia();
    }

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
    let (closure_terminator, call_terminator, terminal_role, terminal_range) =
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
            (
                SyntaxClosureTerminator::Closed,
                SyntaxCallArgumentListTerminator::Closed,
                ExpressionComponentRole::ClosureCloseDelimiter,
                range,
            )
        } else {
            let at = parser.current_offset();
            emit_missing_delimiter(
                parser,
                SyntaxKind::CloseBraceNode,
                SyntaxRole::CloseDelimiter,
            );
            (
                SyntaxClosureTerminator::RecoveredMissing,
                SyntaxCallArgumentListTerminator::RecoveredMissing,
                ExpressionComponentRole::ClosureRecoveryEnd,
                SourceRange::new(at, at),
            )
        };
    closure_components.extend([
        PendingExpressionComponent::new(ExpressionComponentRole::ClosureOpenDelimiter, open),
        PendingExpressionComponent::new(terminal_role, terminal_range),
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
                    terminator: closure_terminator,
                },
            )),
            closure_components,
        ),
    );
    parser.finish();

    let callback = CompletedNode {
        start_event: callback_start,
    };
    let callback_range = parser
        .completed_range(callback.start_event)
        .expect("callback Closure retains one exact source range");
    let terminal_call_role = match call_terminator {
        SyntaxCallArgumentListTerminator::Closed => ExpressionComponentRole::CallArgumentListClose,
        SyntaxCallArgumentListTerminator::RecoveredMissing => {
            ExpressionComponentRole::CallArgumentListRecoveryEnd
        }
    };
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Call(SyntaxCallProjection::CallbackBlock(
                SyntaxCallbackBlockCallProjection::new(
                    completed_slot(parser, callback),
                    call_terminator,
                ),
            )),
            vec![
                PendingExpressionComponent::new(ExpressionComponentRole::CallCallee, callee_range),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgumentListOpen,
                    open,
                ),
                PendingExpressionComponent::new(terminal_call_role, terminal_range),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgument {
                        argument: 0,
                        part: SyntaxCallArgumentPart::Whole,
                    },
                    callback_range,
                ),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgument {
                        argument: 0,
                        part: SyntaxCallArgumentPart::Value,
                    },
                    callback_range,
                ),
            ],
        ),
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
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
        "[" => emit_postfix_bracket(parser, end, left, role),
        "." => emit_select(parser, end, left, role),
        "?" => emit_try(parser, left, role),
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

fn prepare_terminal_call_type_application(
    parser: &ShadowDocumentParser<'_, '_>,
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
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
    start: usize,
    spelling: SyntaxCallTypeApplicationSpelling,
) -> Option<PreparedCallTypeApplication> {
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

    let mut angle_depth = 1_usize;
    let mut paren_depth = 0_usize;
    let mut bracket_depth = 0_usize;
    let mut brace_depth = 0_usize;
    let mut missing_close_candidates = Vec::new();
    for index in open + 1..end {
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
                return prepare_call_type_application_payload(
                    parser,
                    start,
                    spelling,
                    turbofish_separator,
                    open,
                    index,
                    PreparedCallTypeTerminator::InvalidPresent { index },
                    call_open,
                );
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
                    return prepare_call_type_application_payload(
                        parser,
                        start,
                        spelling,
                        turbofish_separator,
                        open,
                        index,
                        PreparedCallTypeTerminator::Closed { index },
                        call_open,
                    );
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
    prepare_call_type_application_payload(
        parser,
        start,
        spelling,
        turbofish_separator,
        open,
        call_open,
        PreparedCallTypeTerminator::RecoveredMissing { call_open },
        call_open,
    )
}

fn prepare_call_type_application_payload(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    spelling: SyntaxCallTypeApplicationSpelling,
    turbofish_separator: Option<usize>,
    open: usize,
    content_end: usize,
    terminator: PreparedCallTypeTerminator,
    call_open: usize,
) -> Option<PreparedCallTypeApplication> {
    let separators = top_level_type_argument_separators(parser, open + 1, content_end);
    let trailing_separator = separators.last().is_some_and(|separator| {
        next_significant_index(parser, separator + 1, content_end).is_none()
    });
    let mut bounds = Vec::with_capacity(separators.len() + 2);
    bounds.push(open + 1);
    bounds.extend(separators.iter().map(|separator| separator + 1));
    bounds.push(content_end);
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
            content_end
        };
        arguments.push(prepare_call_type_argument(parser, raw_start, raw_end));
    }
    if arguments.is_empty() {
        let at = parser
            .token_at(open)
            .expect("prepared type application retains its opening angle")
            .range()
            .end();
        arguments.push(PreparedCallTypeArgument::Missing {
            insertion: SourceRange::new(at, at),
        });
    }
    Some(PreparedCallTypeApplication {
        start,
        spelling,
        turbofish_separator,
        open,
        arguments,
        separators,
        trailing_separator,
        terminator,
        call_open,
    })
}

fn prepare_call_type_argument(
    parser: &ShadowDocumentParser<'_, '_>,
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
    parser: &ShadowDocumentParser<'_, '_>,
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
    parser: &ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
    prepared: PreparedCallTypeApplication,
) -> (EmittedCallTypeApplication, Vec<PendingExpressionComponent>) {
    let open_range = parser
        .token_at(prepared.open)
        .expect("prepared type application retains its opening angle")
        .range();
    let terminator_range = match prepared.terminator {
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
    let whole = SourceRange::new(whole_start, terminator_range.end());
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::CallTypeApplication(SyntaxCallTypeApplicationComponentRole::Whole),
        whole,
    )];
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

    let projections = prepared
        .arguments
        .iter()
        .map(PreparedCallTypeArgument::projection)
        .collect::<Vec<_>>();
    for (ordinal, argument) in prepared.arguments.into_iter().enumerate() {
        let ordinal = u16::try_from(ordinal)
            .expect("document grammar budget bounds Call type argument ordinals");
        let range = argument.range();
        match argument {
            PreparedCallTypeArgument::Present(prepared) => {
                bump_until(parser, prepared.start());
                let _ = emit_prepared_type(parser, SyntaxRole::Type, prepared);
            }
            PreparedCallTypeArgument::InvalidPresent {
                start, end, error, ..
            } => {
                bump_until(parser, start);
                let _ = emit_recovered_type(parser, SyntaxRole::Type, start, end, error);
            }
            PreparedCallTypeArgument::Missing { .. } => {}
        }
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
        if usize::from(ordinal) + 1 < projections.len() {
            let separator = prepared.separators[usize::from(ordinal)];
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
    }
    if prepared.trailing_separator {
        let separator = *prepared
            .separators
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
    if projections.len() == 1
        && matches!(projections[0], SyntaxCallTypeArgumentProjection::Missing)
        && prepared.separators.is_empty()
    {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::CallTypeApplication(
                SyntaxCallTypeApplicationComponentRole::EmptyInsertion,
            ),
            SourceRange::new(open_range.end(), open_range.end()),
        ));
    }
    let terminator = match prepared.terminator {
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
    };
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

fn emit_call(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    let callee_range = parser
        .completed_range(left.start_event)
        .expect("completed Call callee retains one exact source range");
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::CallExpression, role);
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
    receiver: PreparedTypeProjection,
    separator: usize,
    member: usize,
    type_application: Option<PreparedCallTypeApplication>,
    call_open: usize,
    syntax: SyntaxAssociatedCallSyntax,
}

fn emit_associated_call(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> Option<CompletedNode> {
    let prepared = prepare_associated_call(parser, end)?;
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::CallExpression, role);
    let receiver_range = SourceRange::new(
        prepared.receiver.whole().start(),
        prepared.receiver.whole().end(),
    );

    match prepared.syntax {
        SyntaxAssociatedCallSyntax::DotFallback => {
            let value =
                parser.start_projected_owner(SyntaxKind::PathExpression, SyntaxRole::Callee);
            let _ = emit_prepared_type(parser, SyntaxRole::Type, prepared.receiver);
            parser.set_expression_projection(
                value,
                PendingExpressionProjection::new(ExpressionProjection::Path, Vec::new()),
            );
            parser.finish();
        }
        SyntaxAssociatedCallSyntax::ExplicitDoubleColon => {
            let _ = emit_prepared_type(parser, SyntaxRole::Type, prepared.receiver);
        }
    }

    bump_until(parser, prepared.separator);
    let separator_range = parser
        .bump()
        .expect("associated Call retains its separator token")
        .range();
    bump_until(parser, prepared.member);
    let member_token = parser
        .current()
        .expect("associated Call retains its member token");
    let member_range = member_token.range();
    let member = SyntaxName::try_new(parser.text_of(member_token));
    parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
    parser.bump();
    parser.finish();
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
    let projection = match prepared.syntax {
        SyntaxAssociatedCallSyntax::DotFallback => {
            SyntaxParenthesizedCallProjection::unresolved_dot(
                member,
                type_application,
                tail.arguments,
                tail.terminator,
            )
        }
        SyntaxAssociatedCallSyntax::ExplicitDoubleColon => {
            SyntaxParenthesizedCallProjection::associated(
                SyntaxAssociatedCallSyntax::ExplicitDoubleColon,
                member,
                type_application,
                tail.arguments,
                tail.terminator,
            )
        }
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

fn prepare_associated_call(
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
) -> Option<PreparedAssociatedCall> {
    let start = parser.cursor();
    for call_open in start..end {
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
        let member = previous_significant(parser, start, terminal_start)?;
        let member_token = parser.token_at(member)?;
        if !matches!(
            member_token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        ) {
            continue;
        }
        let separator = previous_significant(parser, start, member)?;
        let syntax = match parser
            .token_at(separator)
            .map(|token| parser.text_of(token))
        {
            Some(".") => SyntaxAssociatedCallSyntax::DotFallback,
            Some("::") => SyntaxAssociatedCallSyntax::ExplicitDoubleColon,
            _ => continue,
        };
        let Ok(receiver) = prepare_type(parser, start, separator) else {
            continue;
        };
        if receiver.authored().value().nominal_path().is_none()
            || matches!(syntax, SyntaxAssociatedCallSyntax::ExplicitDoubleColon)
                && !matches!(receiver.authored().value(), TypeRef::Generic { .. })
        {
            continue;
        }
        return Some(PreparedAssociatedCall {
            receiver,
            separator,
            member,
            type_application,
            call_open,
            syntax,
        });
    }
    None
}

fn terminal_type_application_before_call(
    parser: &ShadowDocumentParser<'_, '_>,
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
    parser: &ShadowDocumentParser<'_, '_>,
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

pub(super) struct EmittedParenthesizedCallTail {
    arguments: Vec<SyntaxCallArgumentProjection>,
    terminator: SyntaxCallArgumentListTerminator,
    components: Vec<PendingExpressionComponent>,
}

impl EmittedParenthesizedCallTail {
    pub(super) fn into_parts(
        self,
    ) -> (
        Vec<SyntaxCallArgumentProjection>,
        SyntaxCallArgumentListTerminator,
        Vec<PendingExpressionComponent>,
    ) {
        (self.arguments, self.terminator, self.components)
    }
}

pub(super) fn emit_parenthesized_call_tail(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> EmittedParenthesizedCallTail {
    let open = parser
        .current()
        .expect("postfix Call dispatch retains the opening parenthesis")
        .range();
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ArgumentList, SyntaxRole::Element(0));
    let mut arguments = Vec::new();
    let mut argument_components = Vec::new();
    let mut separators = Vec::new();
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end || parser.at(")") {
            break;
        }
        let ordinal = u16::try_from(arguments.len())
            .expect("document grammar budget keeps Call argument ordinals in u16");
        let argument_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(end);
        let argument = emit_call_argument(parser, argument_end, ordinal);
        arguments.push(argument.projection);
        argument_components.extend(argument.components);
        if parser.at(",") {
            separators.push(
                parser
                    .bump()
                    .expect("Call separator dispatch retains one comma")
                    .range(),
            );
        } else {
            break;
        }
    }
    parser.finish();
    let (terminator, terminator_role, terminator_range) = if parser.at(")") {
        (
            SyntaxCallArgumentListTerminator::Closed,
            ExpressionComponentRole::CallArgumentListClose,
            parser
                .current()
                .expect("closed Call retains one closing parenthesis")
                .range(),
        )
    } else {
        let at = parser.current_offset();
        (
            SyntaxCallArgumentListTerminator::RecoveredMissing,
            ExpressionComponentRole::CallArgumentListRecoveryEnd,
            SourceRange::new(at, at),
        )
    };
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.expression.missing_call_close",
    );
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::CallArgumentListOpen,
        open,
    )];
    components.append(&mut argument_components);
    for (after, separator) in separators.into_iter().enumerate() {
        if after + 1 < arguments.len() {
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::CallArgumentSeparator {
                    following: u16::try_from(after + 1)
                        .expect("Call argument separator ordinal fits u16"),
                },
                separator,
            ));
        } else {
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::CallArgumentTrailingSeparator,
                separator,
            ));
        }
    }
    if arguments.is_empty() {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::CallArgumentListEmptyInsertion,
            SourceRange::new(open.end(), open.end()),
        ));
    }
    components.push(PendingExpressionComponent::new(
        terminator_role,
        terminator_range,
    ));
    EmittedParenthesizedCallTail {
        arguments,
        terminator,
        components,
    }
}

struct EmittedCallArgument {
    projection: SyntaxCallArgumentProjection,
    components: Vec<PendingExpressionComponent>,
}

fn emit_call_argument(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
) -> EmittedCallArgument {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::CallArgument, SyntaxRole::Argument(ordinal));
    let assignment = find_top_level_boundary(parser, parser.cursor(), &["="]).min(end);
    let (projection, mut components) = if assignment < end {
        let name_start = parser.current_offset();
        let name_end_index = trimmed_end(parser, parser.cursor(), assignment);
        let name_end = parser
            .offset_at_token_boundary(name_end_index)
            .expect("Call name end remains at a token boundary");
        let name_range = SourceRange::new(name_start, name_end);
        let source_name = SyntaxName::try_new(&parser.source()[name_range.as_range()]);
        parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
        bump_until(parser, name_end_index);
        parser.finish();
        bump_until(parser, assignment);
        let equals = parser
            .bump()
            .expect("named Call argument retains one equals token")
            .range();
        parser.bump_trivia();
        let value = emit_expression_node(parser, end, SyntaxRole::Operand);
        let value_range = parser
            .completed_range(value.start_event)
            .expect("named Call value retains one exact source range");
        (
            SyntaxCallArgumentProjection::Named {
                name: source_name,
                equals: SyntaxRequiredTokenState::Present,
                value: completed_slot(parser, value),
            },
            vec![
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgument {
                        argument: ordinal,
                        part: SyntaxCallArgumentPart::Name,
                    },
                    name_range,
                ),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgument {
                        argument: ordinal,
                        part: SyntaxCallArgumentPart::Equals,
                    },
                    equals,
                ),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgument {
                        argument: ordinal,
                        part: SyntaxCallArgumentPart::Value,
                    },
                    value_range,
                ),
            ],
        )
    } else {
        let spread = find_top_level_boundary(parser, parser.cursor(), &["..."]).min(end);
        let value = emit_expression_node(parser, spread, SyntaxRole::Operand);
        let value_range = parser
            .completed_range(value.start_event)
            .expect("Call argument value retains one exact source range");
        let value_slot = completed_slot(parser, value);
        bump_until(parser, spread);
        if parser.at("...") {
            let ellipsis = parser
                .bump()
                .expect("spread Call argument retains one ellipsis token")
                .range();
            (
                SyntaxCallArgumentProjection::Spread {
                    value: value_slot,
                    ellipsis: SyntaxRequiredTokenState::Present,
                },
                vec![
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::CallArgument {
                            argument: ordinal,
                            part: SyntaxCallArgumentPart::Value,
                        },
                        value_range,
                    ),
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::CallArgument {
                            argument: ordinal,
                            part: SyntaxCallArgumentPart::Spread,
                        },
                        ellipsis,
                    ),
                ],
            )
        } else {
            (
                SyntaxCallArgumentProjection::Positional { value: value_slot },
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::CallArgument {
                        argument: ordinal,
                        part: SyntaxCallArgumentPart::Value,
                    },
                    value_range,
                )],
            )
        }
    };
    bump_until(parser, end);
    parser.finish();
    let whole = parser
        .completed_range(start_event)
        .expect("Call argument retains one exact whole range");
    components.push(PendingExpressionComponent::new(
        ExpressionComponentRole::CallArgument {
            argument: ordinal,
            part: SyntaxCallArgumentPart::Whole,
        },
        whole,
    ));
    EmittedCallArgument {
        projection,
        components,
    }
}

fn emit_select(
    parser: &mut ShadowDocumentParser<'_, '_>,
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

fn emit_try(
    parser: &mut ShadowDocumentParser<'_, '_>,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    let operand_range = parser
        .completed_range(left.start_event)
        .expect("completed try operand retains one exact source range");
    let operator_range = parser.current().map_or_else(
        || {
            let at = parser.current_offset();
            arcweft_source::SourceRange::new(at, at)
        },
        |token| token.range(),
    );
    let owner = parser.insert_projected_start(left.start_event, SyntaxKind::TryExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Operand);
    if parser.at("?") {
        parser.bump();
    } else {
        emit_missing_delimiter(parser, SyntaxKind::MissingTokenNode, SyntaxRole::Token);
    }
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Try {
                operand: SyntaxExpressionSlot::Authored,
                form: SyntaxTryForm::PostfixQuestion,
            },
            vec![
                PendingExpressionComponent::new(ExpressionComponentRole::Operand, operand_range),
                PendingExpressionComponent::new(ExpressionComponentRole::Operator, operator_range),
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
