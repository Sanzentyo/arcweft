//! Candidate-local typed keyword-statement lowering.

use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use arcweft_lang_syntax::attachment::{
    AttachedCandidateControlLabel, AttachedCandidateKeywordStatement, AttachedCandidateStatement,
};

use crate::final_lowering::StagedHirModuleTransaction;
use crate::final_lowering::name_projection::{name, name_issue, require_attempted_name_limit};
use crate::identity::ScopeId;
use crate::leaf::HirName;
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::HirExprSourceRole;
use crate::stmt::{HirStmtChildRole, HirStmtKind, HirStmtRecoveryIssue};

use super::super::CandidateCursor;

impl StagedHirModuleTransaction<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "this is the exhaustive candidate-local lowering table for the closed keyword-statement family"
    )]
    pub(super) fn lower_candidate_keyword_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        match statement
            .keyword_statement_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?
        {
            AttachedCandidateKeywordStatement::Out { label, value, .. } => {
                let (label, label_recovery) = lower_candidate_label(label)?;
                let value = self.lower_candidate_statement_expression(
                    value,
                    scope,
                    cursor,
                    HirExprSourceRole::Operand,
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
            AttachedCandidateKeywordStatement::Goto { target, .. } => {
                let target = self.lower_candidate_statement_expression(
                    target,
                    scope,
                    cursor,
                    HirExprSourceRole::Target,
                )?;
                let recovery = self.staged_expression_is_poisoned(target)?.then_some(
                    HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Target,
                    },
                );
                Ok((HirStmtKind::Goto { target }, recovery))
            }
            AttachedCandidateKeywordStatement::Defer { expression, .. } => {
                let expression = self.lower_candidate_statement_expression(
                    expression,
                    scope,
                    cursor,
                    HirExprSourceRole::Operand,
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
            AttachedCandidateKeywordStatement::Signal { target, value, .. } => {
                let target = self.lower_candidate_statement_expression(
                    target,
                    scope,
                    cursor,
                    HirExprSourceRole::Target,
                )?;
                let target_poisoned = self.staged_expression_is_poisoned(target)?;
                let value = self.lower_candidate_statement_expression(
                    value,
                    scope,
                    cursor,
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
                Ok((HirStmtKind::Signal { target, value }, recovery))
            }
            AttachedCandidateKeywordStatement::Break { label, value, .. } => {
                let (label, label_recovery) = lower_candidate_label(label)?;
                let value = value
                    .map(|value| {
                        self.lower_candidate_statement_expression(
                            value,
                            scope,
                            cursor,
                            HirExprSourceRole::Operand,
                        )
                    })
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
            AttachedCandidateKeywordStatement::Continue {
                label,
                forbidden_suffix,
                ..
            } => {
                let (label, label_recovery) = lower_candidate_label(label)?;
                let suffix_recovery = forbidden_suffix
                    .is_some()
                    .then_some(HirStmtRecoveryIssue::MalformedContinue);
                Ok((
                    HirStmtKind::Continue { label },
                    label_recovery.or(suffix_recovery),
                ))
            }
        }
    }
}

fn lower_candidate_label(
    label: Option<AttachedCandidateControlLabel<'_>>,
) -> Result<(Option<HirName>, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
    match label.map(AttachedCandidateControlLabel::value) {
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
