//! Required-operand statement final-HIR lowering.

use arcweft_lang_syntax::attachment::node::{
    CloseStatementKind, ReturnStatementKind, WaitStatementKind, YieldStatementKind,
};
use arcweft_lang_syntax::attachment::{RequiredStatementExpressionNode, StatementNode};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::final_lowering::StagedHirModuleTransaction;
use crate::identity::{ExprId, ScopeId, StmtId};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::stmt::{HirStmtChildRole, HirStmtKind, HirStmtRecoveryIssue};

use super::HirStmtRecoveryOperandSlot;

#[derive(Clone, Copy)]
enum RequiredOperandFamily {
    Return,
    Yield,
    Wait { punctuation_recovery: bool },
    Close,
}

impl RequiredOperandFamily {
    const fn missing_slot(self, insertion: usize) -> HirStmtRecoveryOperandSlot {
        match self {
            Self::Return => HirStmtRecoveryOperandSlot::ReturnValue { insertion },
            Self::Yield => HirStmtRecoveryOperandSlot::YieldExpression { insertion },
            Self::Wait { .. } => HirStmtRecoveryOperandSlot::WaitTarget { insertion },
            Self::Close => HirStmtRecoveryOperandSlot::CloseTarget { insertion },
        }
    }

    const fn recovery_role(self) -> HirStmtChildRole {
        match self {
            Self::Wait { .. } | Self::Close => HirStmtChildRole::Target,
            Self::Return | Self::Yield => HirStmtChildRole::Expression,
        }
    }
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_attached_required_operand_statement(
        &mut self,
        attached: &StatementNode,
        owner: StmtId,
        scope: ScopeId,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let (family, operand) = match attached.kind() {
            SyntaxKind::ReturnStatement => {
                let statement = attached
                    .cast::<ReturnStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (
                    RequiredOperandFamily::Return,
                    statement
                        .value()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                )
            }
            SyntaxKind::YieldStatement => {
                let statement = attached
                    .cast::<YieldStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (
                    RequiredOperandFamily::Yield,
                    statement
                        .expression()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                )
            }
            SyntaxKind::WaitStatement => {
                let statement = attached
                    .cast::<WaitStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let punctuation_recovery = statement
                    .has_punctuation_recovery()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (
                    RequiredOperandFamily::Wait {
                        punctuation_recovery,
                    },
                    statement
                        .target()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                )
            }
            SyntaxKind::CloseStatement => {
                let statement = attached
                    .cast::<CloseStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (
                    RequiredOperandFamily::Close,
                    statement
                        .target()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                )
            }
            _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
        };

        let expression =
            self.lower_required_statement_operand(owner, &operand, scope, |insertion| {
                family.missing_slot(insertion)
            })?;
        let expression_poisoned = self.staged_expression_is_poisoned(expression)?;
        let recovery = if expression_poisoned {
            Some(HirStmtRecoveryIssue::RecoveredChild {
                role: family.recovery_role(),
            })
        } else if matches!(
            family,
            RequiredOperandFamily::Wait {
                punctuation_recovery: true
            }
        ) {
            Some(HirStmtRecoveryIssue::MalformedWait)
        } else {
            None
        };
        let kind = match family {
            RequiredOperandFamily::Return => HirStmtKind::Return { value: expression },
            RequiredOperandFamily::Yield => HirStmtKind::Yield { expression },
            RequiredOperandFamily::Wait { .. } => HirStmtKind::Wait { target: expression },
            RequiredOperandFamily::Close => HirStmtKind::Close { target: expression },
        };
        Ok((kind, recovery))
    }

    pub(super) fn lower_required_statement_operand(
        &mut self,
        owner: StmtId,
        operand: &RequiredStatementExpressionNode,
        scope: ScopeId,
        missing_slot: impl FnOnce(usize) -> HirStmtRecoveryOperandSlot,
    ) -> Result<ExprId, HirLowerFailure> {
        match operand {
            RequiredStatementExpressionNode::Expression(expression) => {
                let expression = expression
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                self.lower_attached_expression(&expression, scope)
            }
            RequiredStatementExpressionNode::Missing(missing) => self
                .lower_missing_statement_expression(
                    owner,
                    scope,
                    missing_slot(missing.range().start()),
                ),
        }
    }
}
