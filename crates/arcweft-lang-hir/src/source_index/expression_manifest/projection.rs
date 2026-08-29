//! Payload, child, and recovery projection validation for final HIR expressions.

use std::collections::BTreeSet;

use arcweft_lang_syntax::attachment::{AttachedExpressionChild, AttachedExpressionNode};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxBinaryOperator, SyntaxBorrowKind,
    SyntaxPlaceholderKind, SyntaxPostfixBracketProjection, SyntaxPostfixCandidateFailureKind,
    SyntaxRecordField, SyntaxSelectedMember, SyntaxUnaryOperator,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::name::SyntaxNameIssue;

use super::call::{call_children_match, call_projection_matches};
use super::dialogue_projection::dialogue_application_projection_matches;
use super::expression_component_role;
use super::leaf::{
    id_ref_source_shape, lifetime_projection_matches, literal_projection_matches,
    numeric_sequence_matches, path_projection_matches, resolved_path_projection_matches,
    short_variant_projection_matches, syntax_id_ref_source_shape,
};
use crate::arena::ArenaSnapshot;
use crate::dialogue_application::{
    HirDialogueContentApplication, HirDialogueNodeKind, HirPostfixBracket,
    HirPostfixBracketCandidates, HirPostfixCandidateFailureKind, HirRichTextTagPayload,
};
use crate::expr::{
    HirBinaryOp, HirBorrowKind, HirComputationBlockKind, HirExpr, HirExprKind,
    HirExpressionRecoveryIssue, HirGenericExprIssue, HirNamedBlockName, HirPlaceholderKind,
    HirPoisonState, HirRecordField, HirRecordFieldIssue, HirRecoveryIssue, HirRecoveryOperandSlot,
    HirSelectedMember, HirThreadMode, HirUnaryOp,
};
use crate::identity::{ExprId, SyntheticKey, SyntheticOwner, SyntheticRole, TypeId};
use crate::leaf::{HirIdRefValue, HirPathValue};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::{
    HirExprSourceRole, HirRecordFieldSourcePart, HirSourceIndex, HirSourceQuery, HirSourceSite,
};
use crate::type_ref::HirType;

#[allow(
    clippy::match_same_arms,
    clippy::too_many_lines,
    reason = "one exhaustive payload projection keeps every final expression family and recovery shape explicit"
)]
pub(super) fn expression_payload_matches(
    payload: &HirExprKind,
    attached: &AttachedExpressionNode,
) -> bool {
    match (payload, attached.projection()) {
        (HirExprKind::Unit, ExpressionProjection::Unit) => true,
        (HirExprKind::Literal(actual), ExpressionProjection::Literal(expected)) => {
            literal_projection_matches(actual, expected)
        }
        (HirExprKind::EntityReference(actual), ExpressionProjection::EntityReference(expected)) => {
            matches!(actual, HirIdRefValue::Resolved(_)) == expected.value().is_ok()
                && id_ref_source_shape(actual) == syntax_id_ref_source_shape(expected)
        }
        (HirExprKind::LifetimePath(actual), ExpressionProjection::LifetimePath(expected)) => {
            lifetime_projection_matches(actual, expected)
        }
        (HirExprKind::Path(actual), ExpressionProjection::Path) => {
            match (attached.path(), attached.nominal_path_type()) {
                (Some(expected), None) => path_projection_matches(actual, expected),
                (None, Some(expected)) => matches!(
                    actual,
                    HirPathValue::Resolved(actual)
                        if expected.value().nominal_path().is_some_and(|expected| {
                            super::super::type_projection::hir_path_matches_type_path(actual, expected)
                        })
                ),
                _ => false,
            }
        }
        (HirExprKind::ShortVariant(actual), ExpressionProjection::ShortVariant(expected)) => {
            short_variant_projection_matches(actual, expected)
        }
        (HirExprKind::Placeholder(actual), ExpressionProjection::Placeholder(expected)) => {
            matches!(
                (actual, expected),
                (
                    HirPlaceholderKind::PartialApplication,
                    SyntaxPlaceholderKind::PartialApplication
                ) | (
                    HirPlaceholderKind::PipeLeft,
                    SyntaxPlaceholderKind::PipeLeft
                )
            )
        }
        (HirExprKind::Tuple(actual), ExpressionProjection::Tuple(expected)) => {
            actual.elements().len() == expected.len()
        }
        (HirExprKind::BracketSequence(actual), ExpressionProjection::BracketSequence(expected)) => {
            actual.elements().len() == expected.len()
        }
        (
            HirExprKind::NumericBracketSequence(actual),
            ExpressionProjection::NumericBracketSequence(expected),
        ) => numeric_sequence_matches(actual, expected),
        (HirExprKind::ArrayRepeat(_), ExpressionProjection::ArrayRepeat(expected)) => {
            expected.len() == 2
        }
        (HirExprKind::Call(actual), ExpressionProjection::Call(expected)) => {
            call_projection_matches(actual, expected)
        }
        (HirExprKind::Select(actual), ExpressionProjection::Select(expected)) => {
            matches!(
                (actual.member(), expected),
                (HirSelectedMember::Name(actual), SyntaxSelectedMember::Name(expected))
                    if actual.as_str() == expected.as_str()
            ) || matches!(
                (actual.member(), expected),
                (HirSelectedMember::Missing, SyntaxSelectedMember::Missing)
            )
        }
        (HirExprKind::Index(_), ExpressionProjection::Index(_)) => true,
        (
            HirExprKind::DialogueContentApplication(actual),
            ExpressionProjection::DialogueContentApplication(expected),
        ) => dialogue_application_projection_matches(actual, expected),
        (HirExprKind::PostfixBracket(actual), ExpressionProjection::PostfixBracket(expected)) => {
            postfix_bracket_projection_matches(actual, expected)
        }
        (HirExprKind::Pipe(_), ExpressionProjection::Pipe(expected)) => expected.len() == 2,
        (HirExprKind::Try(_), ExpressionProjection::Try { .. }) => true,
        (HirExprKind::Await(_), ExpressionProjection::Await { .. }) => true,
        (HirExprKind::Borrow(actual), ExpressionProjection::Borrow { kind, .. }) => matches!(
            (actual.kind(), kind),
            (HirBorrowKind::Shared, SyntaxBorrowKind::Shared)
                | (HirBorrowKind::Mutable, SyntaxBorrowKind::Mutable)
        ),
        (HirExprKind::Dereference(_), ExpressionProjection::Dereference { .. }) => true,
        (HirExprKind::Unary(actual), ExpressionProjection::Unary { operator, .. }) => matches!(
            (actual.operator(), operator),
            (HirUnaryOp::Not, SyntaxUnaryOperator::Not)
                | (HirUnaryOp::Negate, SyntaxUnaryOperator::Negate)
        ),
        (
            HirExprKind::Range(actual),
            ExpressionProjection::Range {
                start,
                end,
                inclusive,
            },
        ) => {
            actual.start().is_some() == start.is_some()
                && actual.end().is_some() == end.is_some()
                && actual.inclusive() == *inclusive
        }
        (HirExprKind::Record(actual), ExpressionProjection::Record(expected)) => {
            attached
                .path()
                .is_some_and(|path| resolved_path_projection_matches(actual.path(), path))
                && record_fields_projection_match(actual.fields(), expected)
        }
        (HirExprKind::RecordLiteral(actual), ExpressionProjection::RecordLiteral(expected)) => {
            record_fields_projection_match(actual.fields(), expected)
        }
        (HirExprKind::Closure(actual), ExpressionProjection::Closure(expected)) => {
            actual.parameters().len() == expected.parameters().len()
                && actual.result_type().is_some() == expected.has_result_type()
                && actual
                    .parameters()
                    .iter()
                    .zip(expected.parameters())
                    .all(|(actual, expected)| actual.ty().is_some() == expected.has_type())
        }
        (HirExprKind::Binary(actual), ExpressionProjection::Binary { operator, .. }) => matches!(
            (actual.operator(), operator),
            (HirBinaryOp::Implies, SyntaxBinaryOperator::Implies)
                | (HirBinaryOp::Or, SyntaxBinaryOperator::Or)
                | (HirBinaryOp::And, SyntaxBinaryOperator::And)
                | (HirBinaryOp::In, SyntaxBinaryOperator::In)
                | (HirBinaryOp::Equal, SyntaxBinaryOperator::Equal)
                | (HirBinaryOp::NotEqual, SyntaxBinaryOperator::NotEqual)
                | (
                    HirBinaryOp::GreaterOrEqual,
                    SyntaxBinaryOperator::GreaterOrEqual
                )
                | (HirBinaryOp::LessOrEqual, SyntaxBinaryOperator::LessOrEqual)
                | (HirBinaryOp::Greater, SyntaxBinaryOperator::Greater)
                | (HirBinaryOp::Less, SyntaxBinaryOperator::Less)
                | (HirBinaryOp::Merge, SyntaxBinaryOperator::Merge)
                | (HirBinaryOp::Add, SyntaxBinaryOperator::Add)
                | (HirBinaryOp::Subtract, SyntaxBinaryOperator::Subtract)
                | (HirBinaryOp::Multiply, SyntaxBinaryOperator::Multiply)
                | (HirBinaryOp::Divide, SyntaxBinaryOperator::Divide)
                | (HirBinaryOp::Remainder, SyntaxBinaryOperator::Remainder)
        ),
        (HirExprKind::If(_), ExpressionProjection::If { .. }) => true,
        (HirExprKind::IfLet(actual), ExpressionProjection::IfLet { guard, .. }) => {
            attached.pattern().is_some() && actual.guard().is_some() == guard.is_some()
        }
        (HirExprKind::Match(actual), ExpressionProjection::Match(expected)) => {
            actual.arms().len() == expected.arms().len()
                && attached.match_arms().len() == expected.arms().len()
        }
        (HirExprKind::Block(actual), ExpressionProjection::Block) => {
            attached.block().is_some_and(|block| {
                block
                    .statements()
                    .is_ok_and(|statements| statements.len() == actual.statements().len())
            })
        }
        (HirExprKind::Loop(actual), ExpressionProjection::Loop) => {
            attached.block().is_some_and(|block| {
                block
                    .statements()
                    .is_ok_and(|statements| statements.len() == actual.statements().len())
            })
        }
        (
            HirExprKind::ComputationBlock(actual),
            ExpressionProjection::ComputationBlock(expected),
        ) => {
            matches!(
                (actual.kind(), expected),
                (
                    HirComputationBlockKind::Result,
                    arcweft_lang_syntax::expressions::SyntaxComputationBlockKind::Result
                ) | (
                    HirComputationBlockKind::Option,
                    arcweft_lang_syntax::expressions::SyntaxComputationBlockKind::Option
                ) | (
                    HirComputationBlockKind::Seq,
                    arcweft_lang_syntax::expressions::SyntaxComputationBlockKind::Seq
                ) | (
                    HirComputationBlockKind::Stream,
                    arcweft_lang_syntax::expressions::SyntaxComputationBlockKind::Stream
                )
            ) && attached.block().is_some_and(|block| {
                block
                    .statements()
                    .is_ok_and(|statements| statements.len() == actual.statements().len())
            })
        }
        (HirExprKind::NamedBlock(actual), ExpressionProjection::NamedBlock(expected)) => {
            let name_matches = match (actual.name(), expected) {
                (HirNamedBlockName::Resolved(actual), Ok(expected)) => {
                    actual.as_str() == expected.as_str()
                }
                (
                    HirNamedBlockName::InvalidPresent(_),
                    Err(
                        SyntaxNameIssue::InvalidStart { .. }
                        | SyntaxNameIssue::InvalidContinuation { .. },
                    ),
                ) => true,
                _ => false,
            };
            name_matches
                && attached.block().is_some_and(|block| {
                    block
                        .statements()
                        .is_ok_and(|statements| statements.len() == actual.statements().len())
                })
        }
        (HirExprKind::Choice(actual), ExpressionProjection::Choice) => attached
            .choice()
            .is_some_and(|expected| choice_projection_matches(actual, expected)),
        (HirExprKind::Thread(actual), ExpressionProjection::Thread(expected)) => {
            let mode_matches = matches!(
                (actual.mode(), expected.mode()),
                (
                    HirThreadMode::Attached,
                    arcweft_lang_syntax::expressions::SyntaxThreadMode::Attached
                ) | (
                    HirThreadMode::Detached,
                    arcweft_lang_syntax::expressions::SyntaxThreadMode::Detached
                )
            );
            let name_matches = match (actual.name(), expected.name()) {
                (None, None | Some(Err(_))) => true,
                (Some(actual), Some(Ok(expected))) => actual.as_str() == expected.as_str(),
                _ => false,
            };
            mode_matches
                && name_matches
                && attached.thread().is_some_and(|syntax| {
                    syntax.statement_body().is_ok_and(|body| match body {
                        arcweft_lang_syntax::attachment::AttachedRequiredThreadExpressionBody::Present(body) => {
                            body.items().len() == actual.body().items().len()
                        }
                        arcweft_lang_syntax::attachment::AttachedRequiredThreadExpressionBody::Missing { .. } => {
                            actual.body().items().is_empty()
                        }
                    })
                })
        }
        (HirExprKind::Error(actual), ExpressionProjection::Error) => {
            actual.issue() == HirGenericExprIssue::UnclassifiedSyntax
        }
        _ => false,
    }
}

fn choice_projection_matches(
    actual: &crate::expr::HirChoiceExpr,
    expected: &arcweft_lang_syntax::attachment::AttachedChoiceExpression,
) -> bool {
    actual.id().is_some() == expected.id().is_some()
        && actual.required_expression_slots().len() == expected.required_expression_slots().len()
        && actual.plan().is_some() == expected.plan().is_some()
        && match expected.body() {
            arcweft_lang_syntax::attachment::AttachedRequiredChoiceBody::Present(body) => {
                actual.body().items().len() == body.items().len()
            }
            arcweft_lang_syntax::attachment::AttachedRequiredChoiceBody::Missing(_) => {
                actual.body().items().is_empty()
            }
        }
        && match (actual.plan(), expected.plan()) {
            (None, None) => true,
            (Some(actual), Some(expected)) => match expected.body() {
                arcweft_lang_syntax::attachment::AttachedRequiredChoicePlanBody::Present(body) => {
                    actual.items().len() == body.items().len()
                }
                arcweft_lang_syntax::attachment::AttachedRequiredChoicePlanBody::Missing(_) => {
                    actual.items().is_empty()
                }
            },
            _ => false,
        }
}

fn postfix_bracket_projection_matches(
    actual: &HirPostfixBracket,
    expected: &SyntaxPostfixBracketProjection,
) -> bool {
    match (actual.candidates(), expected) {
        (
            HirPostfixBracketCandidates::Ambiguous { .. },
            SyntaxPostfixBracketProjection::Ambiguous { .. },
        ) => true,
        (
            HirPostfixBracketCandidates::Invalid {
                index: actual_index,
                dialogue: actual_dialogue,
            },
            SyntaxPostfixBracketProjection::Invalid {
                index: expected_index,
                dialogue: expected_dialogue,
            },
        ) => {
            postfix_failure_kind_projection_matches(actual_index.kind(), expected_index.kind())
                && postfix_failure_kind_projection_matches(
                    actual_dialogue.kind(),
                    expected_dialogue.kind(),
                )
        }
        _ => false,
    }
}

const fn postfix_failure_kind_projection_matches(
    actual: HirPostfixCandidateFailureKind,
    expected: SyntaxPostfixCandidateFailureKind,
) -> bool {
    matches!(
        (actual, expected),
        (
            HirPostfixCandidateFailureKind::EmptyPayload,
            SyntaxPostfixCandidateFailureKind::EmptyPayload
        ) | (
            HirPostfixCandidateFailureKind::UnexpectedToken,
            SyntaxPostfixCandidateFailureKind::UnexpectedToken
        ) | (
            HirPostfixCandidateFailureKind::MissingOperand,
            SyntaxPostfixCandidateFailureKind::MissingOperand
        ) | (
            HirPostfixCandidateFailureKind::TrailingToken,
            SyntaxPostfixCandidateFailureKind::TrailingToken
        ) | (
            HirPostfixCandidateFailureKind::InvalidDialogueAtom,
            SyntaxPostfixCandidateFailureKind::InvalidDialogueAtom
        )
    )
}

#[allow(
    clippy::match_same_arms,
    reason = "the record projection keeps distinct authored field forms explicit in one typed matrix"
)]
fn record_fields_projection_match(
    actual: &[HirRecordField],
    expected: &[SyntaxRecordField],
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let mut names = BTreeSet::new();
    actual.iter().zip(expected).all(|(actual, expected)| {
        let source_name = expected.name().as_ref().ok();
        let duplicate = source_name.is_some_and(|name| !names.insert(name.as_str()));
        match (actual, expected, duplicate) {
            (
                HirRecordField::Explicit {
                    name: actual_name, ..
                },
                SyntaxRecordField::Explicit {
                    name: Ok(expected_name),
                    value: arcweft_lang_syntax::expressions::SyntaxExpressionSlot::Authored,
                },
                false,
            ) => actual_name.as_str() == expected_name.as_str(),
            (
                HirRecordField::Shorthand {
                    name: actual_name, ..
                },
                SyntaxRecordField::Shorthand {
                    name: Ok(expected_name),
                },
                false,
            ) => actual_name.as_str() == expected_name.as_str(),
            (
                HirRecordField::Invalid {
                    issue: HirRecordFieldIssue::MissingName,
                },
                SyntaxRecordField::Explicit { name: Err(_), .. },
                false,
            ) => true,
            (
                HirRecordField::Invalid {
                    issue: HirRecordFieldIssue::MissingValue,
                },
                SyntaxRecordField::Explicit {
                    name: Ok(_),
                    value: arcweft_lang_syntax::expressions::SyntaxExpressionSlot::Missing,
                },
                false,
            ) => true,
            (
                HirRecordField::Invalid {
                    issue: HirRecordFieldIssue::DuplicateName,
                },
                SyntaxRecordField::Explicit { name: Ok(_), .. }
                | SyntaxRecordField::Shorthand { name: Ok(_) },
                true,
            ) => true,
            _ => false,
        }
    })
}

#[allow(
    clippy::match_same_arms,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one exhaustive child projection validates the closed expression family against exact owner, scope, and source inputs"
)]
pub(super) fn expression_children_match(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &super::super::block_projection::BlockValidationArenas<'_>,
    local_resolver: &crate::module::HirLocalResolver<'_>,
    types: &ArenaSnapshot<HirType, TypeId>,
    parent: ExprId,
    payload: &HirExpr,
    attached: &AttachedExpressionNode,
) -> bool {
    let expressions = arenas.expressions;
    if let HirExprKind::IfLet(expression) = payload.kind() {
        return super::super::control_projection::if_let_expression_matches(
            parsed, slots, arenas, parent, payload, expression, attached,
        );
    }
    if let HirExprKind::Call(expression) = payload.kind() {
        return call_children_match(
            parsed,
            slots,
            expressions,
            types,
            parent,
            payload,
            expression,
            attached,
        );
    }
    if let HirExprKind::Closure(expression) = payload.kind() {
        return super::super::control_projection::closure_expression_matches(
            parsed, slots, arenas, types, parent, payload, expression, attached,
        );
    }
    if let HirExprKind::Match(expression) = payload.kind() {
        return super::super::match_projection::match_expression_matches(
            parsed, slots, arenas, parent, payload, expression, attached,
        );
    }
    if let HirExprKind::DialogueContentApplication(expression) = payload.kind() {
        return dialogue_application_children_match(
            parsed,
            slots,
            expressions,
            parent,
            payload,
            expression,
            attached,
        );
    }
    if let HirExprKind::PostfixBracket(expression) = payload.kind() {
        return postfix_bracket_children_match(
            parsed,
            slots,
            expressions,
            parent,
            payload,
            expression,
            attached,
        );
    }
    if !composite_parent_state_matches(payload, attached, slots, expressions) {
        return false;
    }
    if matches!(payload.kind(), HirExprKind::Error(_)) {
        return error_recovery_prefix_matches(
            parsed,
            slots,
            expressions,
            parent,
            payload.scope(),
            attached,
        );
    }
    if let HirExprKind::Record(expression) = payload.kind() {
        return record_children_match(
            index,
            parsed,
            slots,
            arenas,
            local_resolver,
            parent,
            payload.scope(),
            expression.fields(),
            attached,
        );
    }
    if let HirExprKind::RecordLiteral(expression) = payload.kind() {
        return record_children_match(
            index,
            parsed,
            slots,
            arenas,
            local_resolver,
            parent,
            payload.scope(),
            expression.fields(),
            attached,
        );
    }
    if let HirExprKind::Range(expression) = payload.kind() {
        let expected_len =
            usize::from(expression.start().is_some()) + usize::from(expression.end().is_some());
        return attached.children().len() == expected_len
            && attached.children().iter().all(|attached_child| {
                let child = match attached_child.ordinal() {
                    0 => expression.start(),
                    1 => expression.end(),
                    _ => None,
                };
                child.is_some_and(|child| {
                    expression_child_matches(
                        parsed,
                        slots,
                        expressions,
                        parent,
                        payload.scope(),
                        attached,
                        attached_child,
                        child,
                    )
                })
            });
    }
    if let HirExprKind::If(expression) = payload.kind() {
        let ExpressionProjection::If { else_branch, .. } = attached.projection() else {
            return false;
        };
        let expected_len = if else_branch.is_some() { 3 } else { 2 };
        if attached.children().len() != expected_len
            || !attached.children().iter().all(|attached_child| {
                matches!(
                    payload.kind().recovery_operand_slot(attached_child.ordinal()),
                    Some(HirRecoveryOperandSlot::Retained(child))
                        if expression_child_matches(
                            parsed,
                            slots,
                            expressions,
                            parent,
                            payload.scope(),
                            attached,
                            attached_child,
                            child,
                        )
                )
            })
        {
            return false;
        }
        if else_branch.is_some() {
            return true;
        }
        let Some(source) = attached.component(ExpressionComponentRole::ElseBranch) else {
            return false;
        };
        return super::super::block_projection::implicit_unit_tail_matches(
            parsed,
            slots,
            expressions,
            parent,
            expression.else_branch(),
            payload.scope(),
            source,
        );
    }
    if let HirExprKind::Block(expression) = payload.kind() {
        return super::super::block_projection::block_expression_matches(
            parsed, slots, arenas, parent, payload, expression, attached,
        );
    }
    if let HirExprKind::ComputationBlock(expression) = payload.kind() {
        return super::super::block_projection::computation_block_expression_matches(
            parsed, slots, arenas, parent, payload, expression, attached,
        );
    }
    if let HirExprKind::NamedBlock(expression) = payload.kind() {
        return super::super::block_projection::named_block_expression_matches(
            parsed, slots, arenas, parent, payload, expression, attached,
        );
    }
    if let HirExprKind::Loop(expression) = payload.kind() {
        return super::super::block_projection::loop_expression_matches(
            parsed, slots, arenas, parent, payload, expression, attached,
        );
    }
    let children = match payload.kind() {
        HirExprKind::Tuple(expression) => expression.elements().to_vec(),
        HirExprKind::BracketSequence(expression) => expression.elements().to_vec(),
        HirExprKind::ArrayRepeat(expression) => vec![expression.value(), expression.length()],
        HirExprKind::Select(expression) => vec![expression.target()],
        HirExprKind::Index(expression) => vec![expression.target(), expression.index()],
        HirExprKind::Pipe(expression) => vec![expression.left(), expression.right()],
        HirExprKind::Try(expression) => vec![expression.operand()],
        HirExprKind::Await(expression) => vec![expression.operand()],
        HirExprKind::Borrow(expression) => vec![expression.operand()],
        HirExprKind::Dereference(expression) => vec![expression.operand()],
        HirExprKind::Unary(expression) => vec![expression.operand()],
        HirExprKind::Binary(expression) => vec![expression.left(), expression.right()],
        HirExprKind::NumericBracketSequence(_) => return attached.children().is_empty(),
        _ => return attached.children().is_empty(),
    };
    children.len() == attached.children().len()
        && attached
            .children()
            .iter()
            .zip(&children)
            .all(|(attached_child, child)| {
                expression_child_matches(
                    parsed,
                    slots,
                    expressions,
                    parent,
                    payload.scope(),
                    attached,
                    attached_child,
                    *child,
                )
            })
}

fn dialogue_application_children_match(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    parent: ExprId,
    payload: &HirExpr,
    application: &HirDialogueContentApplication,
    attached: &AttachedExpressionNode,
) -> bool {
    let Some(target) = attached.children().first() else {
        return false;
    };
    if target.component_role() != ExpressionComponentRole::Target
        || !expression_child_matches(
            parsed,
            slots,
            expressions,
            parent,
            payload.scope(),
            attached,
            target,
            application.target(),
        )
    {
        return false;
    }

    let expected_nested = application
        .content()
        .nodes()
        .iter()
        .filter(|node| matches!(node.kind(), HirDialogueNodeKind::Interpolation(_)))
        .count()
        + application
            .content()
            .tags()
            .iter()
            .filter(|tag| {
                matches!(
                    tag.payload(),
                    HirRichTextTagPayload::FxCall(_)
                        | HirRichTextTagPayload::DialogueCall(_)
                        | HirRichTextTagPayload::Condition(_)
                )
            })
            .count();
    if attached.children().len() != expected_nested + 1 {
        return false;
    }
    let mut seen = BTreeSet::new();
    attached.children()[1..].iter().all(|attached_child| {
        let component_role = attached_child.component_role();
        if !seen.insert(component_role) {
            return false;
        }
        let child = match component_role {
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: arcweft_lang_syntax::expressions::SyntaxDialogueNodeSourcePart::Interpolation,
            } => usize::try_from(ordinal).ok().and_then(|ordinal| {
                application
                    .content()
                    .nodes()
                    .get(ordinal)
                    .and_then(|node| match node.kind() {
                        HirDialogueNodeKind::Interpolation(expression) => Some(*expression),
                        _ => None,
                    })
            }),
            ExpressionComponentRole::RichTextTag {
                tag,
                part: arcweft_lang_syntax::expressions::SyntaxRichTextTagSourcePart::Payload,
            } => usize::try_from(tag).ok().and_then(|tag| {
                application
                    .content()
                    .tags()
                    .get(tag)
                    .and_then(|tag| match tag.payload() {
                        HirRichTextTagPayload::FxCall(expression)
                        | HirRichTextTagPayload::DialogueCall(expression)
                        | HirRichTextTagPayload::Condition(expression) => Some(*expression),
                        HirRichTextTagPayload::Arguments
                        | HirRichTextTagPayload::Marker(_)
                        | HirRichTextTagPayload::None => None,
                    })
            }),
            _ => None,
        };
        child.is_some_and(|child| {
            expression_child_matches(
                parsed,
                slots,
                expressions,
                parent,
                payload.scope(),
                attached,
                attached_child,
                child,
            )
        })
    })
}

fn postfix_bracket_children_match(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    parent: ExprId,
    payload: &HirExpr,
    postfix: &HirPostfixBracket,
    attached: &AttachedExpressionNode,
) -> bool {
    let [target] = attached.children() else {
        return false;
    };
    if target.component_role() != ExpressionComponentRole::Target
        || !expression_child_matches(
            parsed,
            slots,
            expressions,
            parent,
            payload.scope(),
            attached,
            target,
            postfix.target(),
        )
    {
        return false;
    }
    match postfix.candidates() {
        HirPostfixBracketCandidates::Invalid { .. } => true,
        HirPostfixBracketCandidates::Ambiguous { index, dialogue } => {
            postfix_candidate_root_matches(
                slots,
                expressions,
                parent,
                payload.scope(),
                *index,
                SyntheticRole::PostfixIndexCandidateExpression,
                postfix.target(),
            ) && postfix_candidate_root_matches(
                slots,
                expressions,
                parent,
                payload.scope(),
                *dialogue,
                SyntheticRole::DialogueContentCandidateExpression,
                postfix.target(),
            )
        }
    }
}

fn postfix_candidate_root_matches(
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    parent: ExprId,
    parent_scope: crate::identity::ScopeId,
    candidate: ExprId,
    role: SyntheticRole,
    target: ExprId,
) -> bool {
    let Ok(metadata) = slots.resolve_prepared(candidate) else {
        return false;
    };
    let Ok(payload) = expressions.resolve_prepared(slots, candidate) else {
        return false;
    };
    matches!(
        metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(parent)
                && key.role() == role
                && key.ordinal() == 0
    ) && matches!(metadata.source_site(), HirSourceSite::Insertion(_))
        && payload.scope() == parent_scope
        && match (role, payload.kind()) {
            (SyntheticRole::PostfixIndexCandidateExpression, HirExprKind::Index(index)) => {
                index.target() == target
            }
            (
                SyntheticRole::DialogueContentCandidateExpression,
                HirExprKind::DialogueContentApplication(application),
            ) => application.target() == target,
            _ => false,
        }
}

fn error_recovery_prefix_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    parent: ExprId,
    parent_scope: crate::identity::ScopeId,
    attached: &AttachedExpressionNode,
) -> bool {
    match attached.children() {
        [] => true,
        [prefix] if prefix.ordinal() == 0 && prefix.missing().is_none() => {
            let Ok(Some(semantic)) = prefix.authored_semantic() else {
                return false;
            };
            let Some(child) = slots.prepared_source_owner::<ExprId>(semantic.id()) else {
                return false;
            };
            expression_child_matches(
                parsed,
                slots,
                expressions,
                parent,
                parent_scope,
                attached,
                prefix,
                child,
            )
        }
        _ => false,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the record-child validator compares one exact typed owner, scope, field set, attachment, and arena context"
)]
fn record_children_match(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &super::super::block_projection::BlockValidationArenas<'_>,
    local_resolver: &crate::module::HirLocalResolver<'_>,
    parent: ExprId,
    parent_scope: crate::identity::ScopeId,
    fields: &[HirRecordField],
    attached: &AttachedExpressionNode,
) -> bool {
    let expressions = arenas.expressions;
    fields.iter().enumerate().all(|(field, value)| {
        let Ok(field) = u32::try_from(field) else {
            return false;
        };
        let attached_child = attached.children().iter().find(|child| {
            child.component_role()
                == ExpressionComponentRole::RecordField {
                    field,
                    part: arcweft_lang_syntax::expressions::ExpressionRecordFieldPart::Value,
                }
        });
        match value {
            HirRecordField::Explicit { value, .. } => attached_child.is_some_and(|child| {
                child.authored().is_some()
                    && expression_child_matches(
                        parsed,
                        slots,
                        expressions,
                        parent,
                        parent_scope,
                        attached,
                        child,
                        *value,
                    )
            }),
            HirRecordField::Shorthand { name, local } => {
                let query = HirSourceQuery::Expr {
                    owner: parent,
                    role: HirExprSourceRole::RecordField {
                        field,
                        part: HirRecordFieldSourcePart::Name,
                    },
                };
                let use_start = match index.components.get(&query) {
                    Some(HirSourceSite::Span(span)) => span.range().start(),
                    Some(HirSourceSite::Insertion(_)) | None => return false,
                };
                attached_child.is_none()
                    && matches!(
                        local_resolver.lookup(parent_scope, name.as_str(), use_start),
                        Some(crate::scope::LocalLookup::Found(found)) if found == *local
                    )
            }
            HirRecordField::Invalid {
                issue: HirRecordFieldIssue::MissingValue,
            } => {
                let Some(child) = attached_child.filter(|child| child.missing().is_some()) else {
                    return false;
                };
                let Ok(key) = SyntheticKey::try_new(
                    SyntheticOwner::Expr(parent),
                    SyntheticRole::RecoveryOperand,
                    field,
                ) else {
                    return false;
                };
                let Ok(Some(value)) = slots.resolve_prepared_synthetic::<ExprId>(key) else {
                    return false;
                };
                expression_child_matches(
                    parsed,
                    slots,
                    expressions,
                    parent,
                    parent_scope,
                    attached,
                    child,
                    value,
                )
            }
            HirRecordField::Invalid { .. } => true,
        }
    })
}

fn composite_parent_state_matches(
    payload: &HirExpr,
    attached: &AttachedExpressionNode,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
) -> bool {
    let record_fields = match payload.kind() {
        HirExprKind::Record(expression) => Some(expression.fields()),
        HirExprKind::RecordLiteral(expression) => Some(expression.fields()),
        _ => None,
    };
    if let Some(fields) = record_fields {
        let Ok(expected) = record_parent_recovery(fields, slots, expressions) else {
            return false;
        };
        return poison_state_matches(payload.state(), expected);
    }
    if let HirExprKind::Select(expression) = payload.kind() {
        let Some(attached_target) = attached.children().first() else {
            return false;
        };
        if attached.children().len() != 1 || attached_target.ordinal() != 0 {
            return false;
        }
        let Ok(target) = expressions.resolve_prepared(slots, expression.target()) else {
            return false;
        };
        let expected = if target.is_poisoned() {
            Some(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild {
                    role: HirExprSourceRole::Target,
                },
            ))
        } else if matches!(expression.member(), HirSelectedMember::Missing) {
            Some(HirRecoveryIssue::MissingOperand {
                role: HirExprSourceRole::SelectedMember,
            })
        } else {
            None
        };
        return poison_state_matches(payload.state(), expected);
    }
    let composite = matches!(
        payload.kind(),
        HirExprKind::Tuple(_)
            | HirExprKind::BracketSequence(_)
            | HirExprKind::ArrayRepeat(_)
            | HirExprKind::Index(_)
            | HirExprKind::Pipe(_)
            | HirExprKind::Try(_)
            | HirExprKind::Await(_)
            | HirExprKind::Borrow(_)
            | HirExprKind::Dereference(_)
            | HirExprKind::Unary(_)
            | HirExprKind::Range(_)
            | HirExprKind::Binary(_)
            | HirExprKind::If(_)
    );
    if !composite {
        return match payload.kind() {
            HirExprKind::Error(error) => matches!(
                payload.state(),
                HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::Generic(actual),
                )) if *actual == error.issue()
            ),
            _ => true,
        };
    }
    let mut expected = None;
    for child in attached.children() {
        let Some(role) = expression_component_role(attached.projection(), child.component_role())
        else {
            return false;
        };
        if child.missing().is_some() {
            expected = Some(HirRecoveryIssue::MissingOperand { role });
            break;
        }
        let Some(HirRecoveryOperandSlot::Retained(child_id)) =
            payload.kind().recovery_operand_slot(child.ordinal())
        else {
            return false;
        };
        let Ok(child_payload) = expressions.resolve_prepared(slots, child_id) else {
            return false;
        };
        if child_payload.is_poisoned() {
            expected = Some(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild { role },
            ));
            break;
        }
    }
    poison_state_matches(payload.state(), expected)
}

fn record_parent_recovery(
    fields: &[HirRecordField],
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
) -> Result<Option<HirRecoveryIssue>, ()> {
    for (field, value) in fields.iter().enumerate() {
        let field = u32::try_from(field).map_err(|_| ())?;
        let role = HirExprSourceRole::RecordField {
            field,
            part: HirRecordFieldSourcePart::Value,
        };
        match value {
            HirRecordField::Explicit { value, .. } => {
                let child = expressions
                    .resolve_prepared(slots, *value)
                    .map_err(|_| ())?;
                if child.is_poisoned() {
                    return Ok(Some(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild { role },
                    )));
                }
            }
            HirRecordField::Shorthand { .. } => {}
            HirRecordField::Invalid {
                issue: HirRecordFieldIssue::MissingValue,
            } => return Ok(Some(HirRecoveryIssue::MissingOperand { role })),
            HirRecordField::Invalid {
                issue:
                    HirRecordFieldIssue::MissingName
                    | HirRecordFieldIssue::DuplicateName
                    | HirRecordFieldIssue::ForeignChild,
            } => {
                return Ok(Some(HirRecoveryIssue::InvalidName(
                    crate::leaf::HirNameInvariantError::InvalidIdentifier,
                )));
            }
        }
    }
    Ok(None)
}

pub(in crate::source_index) fn poison_state_matches(
    actual: &HirPoisonState,
    expected: Option<HirRecoveryIssue>,
) -> bool {
    match (actual, expected) {
        (HirPoisonState::Clean, None) => true,
        (HirPoisonState::Poisoned(actual), Some(expected)) => actual == &expected,
        _ => false,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the child validator compares one exact parent/child identity, scope, role, attachment, and arena context"
)]
pub(in crate::source_index) fn expression_child_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    parent: ExprId,
    parent_scope: crate::identity::ScopeId,
    parent_attached: &AttachedExpressionNode,
    attached: &AttachedExpressionChild,
    child: ExprId,
) -> bool {
    let Some(role) =
        expression_component_role(parent_attached.projection(), attached.component_role())
    else {
        return false;
    };
    let Ok(metadata) = slots.resolve_prepared(child) else {
        return false;
    };
    let Ok(payload) = expressions.resolve_prepared(slots, child) else {
        return false;
    };
    if payload.scope() != parent_scope {
        return false;
    }
    match attached {
        AttachedExpressionChild::Authored { .. } => {
            let Ok(Some(semantic)) = attached.authored_semantic() else {
                return false;
            };
            matches!(metadata.origin(), HirOrigin::Source(source) if source.syntax() == semantic.id())
                && metadata.source_site() == &HirSourceSite::Span(semantic.whole_source_span())
        }
        AttachedExpressionChild::Missing { ordinal, .. } => {
            let Ok(expected_site) =
                HirSourceSite::from_attached_span(parsed.document(), &attached.source_span())
            else {
                return false;
            };
            matches!(
                metadata.origin(),
                HirOrigin::Synthetic(key)
                    if key.owner() == SyntheticOwner::Expr(parent)
                        && key.role() == SyntheticRole::RecoveryOperand
                        && key.ordinal() == *ordinal
            ) && metadata.source_site() == &expected_site
                && matches!(
                    (payload.kind(), payload.state()),
                    (
                        HirExprKind::Error(error),
                        HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
                            role: actual,
                        })
                    ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure
                        && *actual == role
                )
        }
    }
}
