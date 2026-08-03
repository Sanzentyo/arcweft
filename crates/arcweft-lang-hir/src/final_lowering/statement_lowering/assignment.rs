//! Assignment-family final-HIR lowering.

use arcweft_lang_syntax::attachment::node::{AssignmentStatementKind, LifetimeSetStatementKind};
use arcweft_lang_syntax::attachment::{RequiredStatementExpressionNode, StatementNode};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::final_lowering::StagedHirModuleTransaction;
use crate::identity::{ExprId, ScopeId, StmtId};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::HirExprSourceRole;
use crate::stmt::{HirStmtChildRole, HirStmtKind, HirStmtRecoveryIssue};

#[derive(Clone, Copy)]
enum AssignmentFamily {
    Assignment,
    LifetimeSet,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_attached_assignment_statement(
        &mut self,
        attached: &StatementNode,
        owner: StmtId,
        scope: ScopeId,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let (family, target, value) = match attached.kind() {
            SyntaxKind::AssignmentStatement => {
                let statement = attached
                    .cast::<AssignmentStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (
                    AssignmentFamily::Assignment,
                    statement
                        .target()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                    statement
                        .value()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                )
            }
            SyntaxKind::LifetimeSetStatement => {
                let statement = attached
                    .cast::<LifetimeSetStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (
                    AssignmentFamily::LifetimeSet,
                    statement
                        .target()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                    statement
                        .value()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                )
            }
            _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
        };

        let target = self.lower_attached_assignment_operand(
            owner,
            target,
            scope,
            0,
            HirExprSourceRole::Target,
        )?;
        if matches!(family, AssignmentFamily::Assignment) {
            self.upgrade_direct_reassignment_capture(target)?;
        }
        let target_poisoned = self.staged_expression_is_poisoned(target)?;
        let value = self.lower_attached_assignment_operand(
            owner,
            value,
            scope,
            1,
            HirExprSourceRole::Operand,
        )?;
        let value_poisoned = self.staged_expression_is_poisoned(value)?;
        let recovery = if target_poisoned {
            Some(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Target,
            })
        } else if value_poisoned {
            Some(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Initializer,
            })
        } else {
            None
        };
        let kind = match family {
            AssignmentFamily::Assignment => HirStmtKind::Assign { target, value },
            AssignmentFamily::LifetimeSet => HirStmtKind::LifetimeSet { target, value },
        };
        Ok((kind, recovery))
    }

    fn lower_attached_assignment_operand(
        &mut self,
        owner: StmtId,
        operand: RequiredStatementExpressionNode,
        scope: ScopeId,
        ordinal: u32,
        role: HirExprSourceRole,
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
                    missing.range().start(),
                    ordinal,
                    role,
                ),
        }
    }
}
