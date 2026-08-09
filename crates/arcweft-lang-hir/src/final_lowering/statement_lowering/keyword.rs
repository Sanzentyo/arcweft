//! Typed keyword-statement lowering into final HIR.

use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use arcweft_lang_syntax::attachment::node::{
    BreakStatementKind, ContinueStatementKind, DeferStatementKind, GotoStatementKind,
    OutStatementKind, SignalStatementKind,
};
use arcweft_lang_syntax::attachment::{
    AttachedControlLabel, RequiredStatementExpressionNode, StatementNode,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::final_lowering::StagedHirModuleTransaction;
use crate::final_lowering::name_projection::{name, name_issue, require_attempted_name_limit};
use crate::identity::{ExprId, ScopeId, StmtId};
use crate::leaf::HirName;
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::stmt::{HirStmtChildRole, HirStmtKind, HirStmtRecoveryIssue};

use super::HirStmtRecoveryOperandSlot;

impl StagedHirModuleTransaction<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "the closed keyword-statement family is one exhaustive typed lowering matrix"
    )]
    pub(super) fn lower_attached_keyword_statement(
        &mut self,
        attached: &StatementNode,
        owner: StmtId,
        scope: ScopeId,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        match attached.kind() {
            SyntaxKind::OutStatement => {
                let statement = attached
                    .cast::<OutStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (label, label_recovery) = lower_label(statement.label())?;
                let value = self.lower_keyword_required_expression(
                    owner,
                    statement.value(),
                    scope,
                    |insertion| HirStmtRecoveryOperandSlot::OutValue { insertion },
                )?;
                let value_recovery = self.staged_expression_is_poisoned(value)?.then_some(
                    HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    },
                );
                Ok((
                    HirStmtKind::Out { label, value },
                    label_recovery.or(value_recovery),
                ))
            }
            SyntaxKind::GotoStatement => {
                let statement = attached
                    .cast::<GotoStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let target = self.lower_keyword_required_expression(
                    owner,
                    statement.target(),
                    scope,
                    |insertion| HirStmtRecoveryOperandSlot::GotoTarget { insertion },
                )?;
                let recovery = self.staged_expression_is_poisoned(target)?.then_some(
                    HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Target,
                    },
                );
                Ok((HirStmtKind::Goto { target }, recovery))
            }
            SyntaxKind::DeferStatement => {
                let statement = attached
                    .cast::<DeferStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let expression = self.lower_keyword_required_expression(
                    owner,
                    statement.expression(),
                    scope,
                    |insertion| HirStmtRecoveryOperandSlot::DeferExpression { insertion },
                )?;
                let recovery = self.staged_expression_is_poisoned(expression)?.then_some(
                    HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    },
                );
                Ok((
                    HirStmtKind::Defer {
                        outcome: DeferOutcome::Always,
                        expression,
                    },
                    recovery,
                ))
            }
            SyntaxKind::SignalStatement => {
                let statement = attached
                    .cast::<SignalStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let target = self.lower_keyword_required_expression(
                    owner,
                    statement.target(),
                    scope,
                    |insertion| HirStmtRecoveryOperandSlot::SignalTarget { insertion },
                )?;
                let target_poisoned = self.staged_expression_is_poisoned(target)?;
                let value = self.lower_keyword_required_expression(
                    owner,
                    statement.value(),
                    scope,
                    |insertion| HirStmtRecoveryOperandSlot::SignalValue { insertion },
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
                Ok((HirStmtKind::Signal { target, value }, recovery))
            }
            SyntaxKind::BreakStatement => {
                let statement = attached
                    .cast::<BreakStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (label, label_recovery) = lower_label(statement.label())?;
                let value = statement
                    .value()
                    .map(|value| self.lower_attached_expression(value, scope))
                    .transpose()?;
                let value_recovery = value
                    .map(|value| self.staged_expression_is_poisoned(value))
                    .transpose()?
                    .unwrap_or(false)
                    .then_some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    });
                Ok((
                    HirStmtKind::Break { label, value },
                    label_recovery.or(value_recovery),
                ))
            }
            SyntaxKind::ContinueStatement => {
                let statement = attached
                    .cast::<ContinueStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (label, label_recovery) = lower_label(statement.label())?;
                let suffix_recovery = statement
                    .forbidden_suffix()
                    .is_some()
                    .then_some(HirStmtRecoveryIssue::MalformedContinue);
                Ok((
                    HirStmtKind::Continue { label },
                    label_recovery.or(suffix_recovery),
                ))
            }
            _ => Err(HirInvariantFailure::InvalidArenaCommit.into()),
        }
    }

    fn lower_keyword_required_expression(
        &mut self,
        owner: StmtId,
        expression: &RequiredStatementExpressionNode,
        scope: ScopeId,
        missing_slot: impl FnOnce(usize) -> HirStmtRecoveryOperandSlot,
    ) -> Result<ExprId, HirLowerFailure> {
        match expression {
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

fn lower_label(
    label: Option<&AttachedControlLabel>,
) -> Result<(Option<HirName>, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
    match label.map(AttachedControlLabel::value) {
        None => Ok((None, None)),
        Some(Ok(value)) => Ok((Some(name(value)?), None)),
        Some(Err(issue)) => {
            require_attempted_name_limit(issue)?;
            Ok((
                None,
                Some(HirStmtRecoveryIssue::InvalidControlLabel(name_issue(issue))),
            ))
        }
    }
}
