//! Required-operand statement source-freeze evidence.

use arcweft_lang_syntax::attachment::node::{
    CloseStatementKind, ReturnStatementKind, SelectStatementKind, WaitStatementKind,
    YieldStatementKind,
};
use arcweft_lang_syntax::attachment::{RequiredStatementExpressionNode, StatementNode};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::arena::ArenaSnapshot;
use crate::expr::HirExpr;
use crate::identity::{ExprId, ScopeId, StmtId};
use crate::slot::SlotSnapshot;
use crate::source_index::HirStmtRecoveryOperandSlot;
use crate::stmt::{
    HirSelectStmt, HirStmtChildRole, HirStmtKind, HirStmtPoisonState, HirStmtRecoveryIssue,
};

use super::{StatementEvidence, missing_statement_expression_matches, source_expression_matches};

#[derive(Clone, Copy)]
enum RequiredOperandFamily {
    Return,
    Yield,
    Wait { punctuation_recovery: bool },
    Close,
    Select,
}

impl RequiredOperandFamily {
    const fn missing_slot(self, insertion: usize) -> HirStmtRecoveryOperandSlot {
        match self {
            Self::Return => HirStmtRecoveryOperandSlot::ReturnValue { insertion },
            Self::Yield => HirStmtRecoveryOperandSlot::YieldExpression { insertion },
            Self::Wait { .. } => HirStmtRecoveryOperandSlot::WaitTarget { insertion },
            Self::Close => HirStmtRecoveryOperandSlot::CloseTarget { insertion },
            Self::Select => HirStmtRecoveryOperandSlot::SelectOperand { insertion },
        }
    }

    const fn recovery_role(self) -> HirStmtChildRole {
        match self {
            Self::Wait { .. } | Self::Close => HirStmtChildRole::Target,
            Self::Return | Self::Yield | Self::Select => HirStmtChildRole::Expression,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn required_operand_statement_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statement: StmtId,
    attached: &StatementNode,
    payload: &HirStmtKind,
    scope: ScopeId,
) -> Option<StatementEvidence> {
    let (family, operand, owner) = match (attached.kind(), payload) {
        (SyntaxKind::ReturnStatement, HirStmtKind::Return { value }) => {
            let attached = attached.cast::<ReturnStatementKind>().ok()?;
            (
                RequiredOperandFamily::Return,
                attached.value().ok()?,
                *value,
            )
        }
        (SyntaxKind::YieldStatement, HirStmtKind::Yield { expression }) => {
            let attached = attached.cast::<YieldStatementKind>().ok()?;
            (
                RequiredOperandFamily::Yield,
                attached.expression().ok()?,
                *expression,
            )
        }
        (SyntaxKind::WaitStatement, HirStmtKind::Wait { target }) => {
            let attached = attached.cast::<WaitStatementKind>().ok()?;
            (
                RequiredOperandFamily::Wait {
                    punctuation_recovery: attached.has_punctuation_recovery().ok()?,
                },
                attached.target().ok()?,
                *target,
            )
        }
        (SyntaxKind::CloseStatement, HirStmtKind::Close { target }) => {
            let attached = attached.cast::<CloseStatementKind>().ok()?;
            (
                RequiredOperandFamily::Close,
                attached.target().ok()?,
                *target,
            )
        }
        (SyntaxKind::SelectStatement, HirStmtKind::Select(HirSelectStmt::Operand(expression))) => {
            let attached = attached.cast::<SelectStatementKind>().ok()?;
            (
                RequiredOperandFamily::Select,
                attached.expression().ok()?,
                *expression,
            )
        }
        _ => return None,
    };

    if !required_statement_expression_matches(
        parsed,
        slots,
        expressions,
        statement,
        owner,
        operand,
        scope,
        family,
    ) {
        return None;
    }
    let expression_poisoned = expressions
        .resolve_prepared(slots, owner)
        .is_ok_and(HirExpr::is_poisoned);
    let state = if expression_poisoned {
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: family.recovery_role(),
        })
    } else if matches!(
        family,
        RequiredOperandFamily::Wait {
            punctuation_recovery: true
        }
    ) {
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MalformedWait)
    } else {
        HirStmtPoisonState::Clean
    };
    Some(StatementEvidence {
        locals: Box::new([]),
        state,
    })
}

#[allow(clippy::too_many_arguments)]
fn required_statement_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statement: StmtId,
    owner: ExprId,
    attached: RequiredStatementExpressionNode,
    scope: ScopeId,
    family: RequiredOperandFamily,
) -> bool {
    match attached {
        RequiredStatementExpressionNode::Expression(attached) => {
            attached.semantic().is_ok_and(|attached| {
                source_expression_matches(slots, expressions, owner, &attached, scope)
            })
        }
        RequiredStatementExpressionNode::Missing(missing) => missing_statement_expression_matches(
            parsed,
            slots,
            expressions,
            statement,
            owner,
            scope,
            family.missing_slot(missing.range().start()),
        ),
    }
}
