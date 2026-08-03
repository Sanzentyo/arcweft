//! Candidate-local keyword-statement source-freeze evidence.

use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use arcweft_lang_syntax::attachment::{
    AttachedCandidateControlLabel, AttachedCandidateKeywordStatement, AttachedCandidateStatement,
};

use crate::final_lowering::name_projection::{name, name_issue};
use crate::identity::ScopeId;
use crate::leaf::HirName;
use crate::source_index::HirExprSourceRole;
use crate::stmt::{HirStmtChildRole, HirStmtKind, HirStmtPoisonState, HirStmtRecoveryIssue};

use super::super::CandidateValidationCursor;

impl CandidateValidationCursor<'_> {
    pub(super) fn validate_keyword_statement(
        &mut self,
        source: AttachedCandidateStatement<'_>,
        scope: ScopeId,
        payload: &HirStmtKind,
    ) -> Option<(HirStmtPoisonState, bool)> {
        let (matches, recovery) = match source.keyword_statement_view()? {
            AttachedCandidateKeywordStatement::Out { label, value, .. } => {
                let (expected_label, label_recovery) = label_evidence(label)?;
                let value =
                    self.validate_statement_expression(value, scope, HirExprSourceRole::Operand)?;
                let value_recovery =
                    value
                        .poisoned
                        .then_some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Initializer,
                        });
                (
                    matches!(payload, HirStmtKind::Out { label, value: actual }
                        if label == &expected_label && *actual == value.id),
                    label_recovery.or(value_recovery),
                )
            }
            AttachedCandidateKeywordStatement::Goto { target, .. } => {
                let target =
                    self.validate_statement_expression(target, scope, HirExprSourceRole::Target)?;
                let recovery = target
                    .poisoned
                    .then_some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Target,
                    });
                (
                    matches!(payload, HirStmtKind::Goto { target: actual }
                        if *actual == target.id),
                    recovery,
                )
            }
            AttachedCandidateKeywordStatement::Defer { expression, .. } => {
                let expression = self.validate_statement_expression(
                    expression,
                    scope,
                    HirExprSourceRole::Operand,
                )?;
                let recovery =
                    expression
                        .poisoned
                        .then_some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Initializer,
                        });
                (
                    matches!(payload, HirStmtKind::Defer {
                        outcome: DeferOutcome::Always,
                        expression: actual,
                    } if *actual == expression.id),
                    recovery,
                )
            }
            AttachedCandidateKeywordStatement::Signal {
                target,
                value,
                arrow_recovery: _,
                ..
            } => {
                let target =
                    self.validate_statement_expression(target, scope, HirExprSourceRole::Target)?;
                let value =
                    self.validate_statement_expression(value, scope, HirExprSourceRole::Operand)?;
                let recovery = if target.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Target,
                    })
                } else if value.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    })
                } else {
                    None
                };
                (
                    matches!(payload, HirStmtKind::Signal {
                        target: actual_target,
                        value: actual_value,
                    } if *actual_target == target.id && *actual_value == value.id),
                    recovery,
                )
            }
            AttachedCandidateKeywordStatement::Break { label, value, .. } => {
                let (expected_label, label_recovery) = label_evidence(label)?;
                let value = match value {
                    Some(value) => Some(self.validate_statement_expression(
                        value,
                        scope,
                        HirExprSourceRole::Operand,
                    )?),
                    None => None,
                };
                let value_recovery = value.is_some_and(|value| value.poisoned).then_some(
                    HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    },
                );
                (
                    matches!(payload, HirStmtKind::Break { label, value: actual }
                        if label == &expected_label
                            && *actual == value.map(|value| value.id)),
                    label_recovery.or(value_recovery),
                )
            }
            AttachedCandidateKeywordStatement::Continue {
                label,
                forbidden_suffix,
                ..
            } => {
                let (expected_label, label_recovery) = label_evidence(label)?;
                let suffix_recovery = forbidden_suffix
                    .is_some()
                    .then_some(HirStmtRecoveryIssue::MalformedContinue);
                (
                    matches!(payload, HirStmtKind::Continue { label }
                        if label == &expected_label),
                    label_recovery.or(suffix_recovery),
                )
            }
        };
        Some((
            recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
            matches,
        ))
    }
}

fn label_evidence(
    source: Option<AttachedCandidateControlLabel<'_>>,
) -> Option<(Option<HirName>, Option<HirStmtRecoveryIssue>)> {
    match source.map(AttachedCandidateControlLabel::value) {
        None => Some((None, None)),
        Some(Ok(value)) => Some((Some(name(value).ok()?), None)),
        Some(Err(issue)) => Some((
            None,
            Some(HirStmtRecoveryIssue::InvalidControlLabel(name_issue(issue))),
        )),
    }
}
