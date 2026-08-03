//! Typed keyword-statement source-freeze evidence.

use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use arcweft_lang_syntax::attachment::node::{
    BreakStatementKind, ContinueStatementKind, DeferStatementKind, GotoStatementKind,
    OutStatementKind, SignalStatementKind,
};
use arcweft_lang_syntax::attachment::{
    AttachedControlLabel, RequiredStatementExpressionNode, StatementNode,
};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::arena::ArenaSnapshot;
use crate::expr::{HirExpr, HirPoisonState};
use crate::final_lowering::name_projection::{name, name_issue};
use crate::identity::{ExprId, ScopeId, StmtId};
use crate::leaf::HirName;
use crate::slot::SlotSnapshot;
use crate::source_index::HirExprSourceRole;
use crate::stmt::{HirStmtChildRole, HirStmtKind, HirStmtPoisonState, HirStmtRecoveryIssue};

use super::{StatementEvidence, missing_statement_expression_matches, source_expression_matches};

#[allow(clippy::too_many_arguments)]
pub(super) fn keyword_statement_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statement: StmtId,
    attached: &StatementNode,
    payload: &HirStmtKind,
    scope: ScopeId,
) -> Option<StatementEvidence> {
    let (matches, recovery) = match (attached.kind(), payload) {
        (SyntaxKind::OutStatement, HirStmtKind::Out { label, value }) => {
            let source = attached.cast::<OutStatementKind>().ok()?.semantics().ok()?;
            let (expected_label, label_recovery) = label_evidence(source.label())?;
            let value_matches = required_expression_matches(
                parsed,
                slots,
                expressions,
                statement,
                *value,
                source.value(),
                scope,
                0,
                HirExprSourceRole::Operand,
            );
            let value_recovery = expression_is_poisoned(slots, expressions, *value).then_some(
                HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Initializer,
                },
            );
            (
                label == &expected_label && value_matches,
                label_recovery.or(value_recovery),
            )
        }
        (SyntaxKind::GotoStatement, HirStmtKind::Goto { target }) => {
            let source = attached
                .cast::<GotoStatementKind>()
                .ok()?
                .semantics()
                .ok()?;
            let target_matches = required_expression_matches(
                parsed,
                slots,
                expressions,
                statement,
                *target,
                source.target(),
                scope,
                0,
                HirExprSourceRole::Target,
            );
            let recovery = expression_is_poisoned(slots, expressions, *target).then_some(
                HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Target,
                },
            );
            (target_matches, recovery)
        }
        (
            SyntaxKind::DeferStatement,
            HirStmtKind::Defer {
                outcome: DeferOutcome::Always,
                expression,
            },
        ) => {
            let source = attached
                .cast::<DeferStatementKind>()
                .ok()?
                .semantics()
                .ok()?;
            let expression_matches = required_expression_matches(
                parsed,
                slots,
                expressions,
                statement,
                *expression,
                source.expression(),
                scope,
                0,
                HirExprSourceRole::Operand,
            );
            let recovery = expression_is_poisoned(slots, expressions, *expression).then_some(
                HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Initializer,
                },
            );
            (expression_matches, recovery)
        }
        (SyntaxKind::SignalStatement, HirStmtKind::Signal { target, value }) => {
            let source = attached
                .cast::<SignalStatementKind>()
                .ok()?
                .semantics()
                .ok()?;
            let target_matches = required_expression_matches(
                parsed,
                slots,
                expressions,
                statement,
                *target,
                source.target(),
                scope,
                0,
                HirExprSourceRole::Target,
            );
            let value_matches = required_expression_matches(
                parsed,
                slots,
                expressions,
                statement,
                *value,
                source.value(),
                scope,
                1,
                HirExprSourceRole::Operand,
            );
            let recovery = if expression_is_poisoned(slots, expressions, *target) {
                Some(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Target,
                })
            } else if expression_is_poisoned(slots, expressions, *value) {
                Some(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Initializer,
                })
            } else {
                None
            };
            // Constructing the typed view above also verifies the exact optional
            // Recovery(0) arrow node; it is source evidence, not a second HIR issue.
            let _ = source.arrow_recovery();
            (target_matches && value_matches, recovery)
        }
        (SyntaxKind::BreakStatement, HirStmtKind::Break { label, value }) => {
            let source = attached
                .cast::<BreakStatementKind>()
                .ok()?
                .semantics()
                .ok()?;
            let (expected_label, label_recovery) = label_evidence(source.label())?;
            let value_matches = match (value, source.value()) {
                (None, None) => true,
                (Some(owner), Some(source)) => {
                    source_expression_matches(slots, expressions, *owner, source, scope)
                }
                _ => false,
            };
            let value_recovery = value
                .is_some_and(|owner| expression_is_poisoned(slots, expressions, owner))
                .then_some(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Initializer,
                });
            (
                label == &expected_label && value_matches,
                label_recovery.or(value_recovery),
            )
        }
        (SyntaxKind::ContinueStatement, HirStmtKind::Continue { label }) => {
            let source = attached
                .cast::<ContinueStatementKind>()
                .ok()?
                .semantics()
                .ok()?;
            let (expected_label, label_recovery) = label_evidence(source.label())?;
            let suffix_recovery = source
                .forbidden_suffix()
                .is_some()
                .then_some(HirStmtRecoveryIssue::MalformedContinue);
            (label == &expected_label, label_recovery.or(suffix_recovery))
        }
        _ => return None,
    };

    matches.then_some(StatementEvidence {
        locals: Box::new([]),
        state: recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
    })
}

#[allow(clippy::too_many_arguments)]
fn required_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statement: StmtId,
    owner: ExprId,
    source: &RequiredStatementExpressionNode,
    scope: ScopeId,
    ordinal: u32,
    role: HirExprSourceRole,
) -> bool {
    match source {
        RequiredStatementExpressionNode::Expression(source) => {
            source.semantic().is_ok_and(|source| {
                source_expression_matches(slots, expressions, owner, &source, scope)
            })
        }
        RequiredStatementExpressionNode::Missing(missing) => missing_statement_expression_matches(
            parsed,
            slots,
            expressions,
            statement,
            owner,
            scope,
            missing.range().start(),
            ordinal,
            role,
        ),
    }
}

fn expression_is_poisoned(
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    owner: ExprId,
) -> bool {
    expressions
        .resolve_prepared(slots, owner)
        .is_ok_and(|expression| matches!(expression.state(), HirPoisonState::Poisoned(_)))
}

fn label_evidence(
    source: Option<&AttachedControlLabel>,
) -> Option<(Option<HirName>, Option<HirStmtRecoveryIssue>)> {
    match source.map(AttachedControlLabel::value) {
        None => Some((None, None)),
        Some(Ok(value)) => Some((Some(name(value).ok()?), None)),
        Some(Err(issue)) => Some((
            None,
            Some(HirStmtRecoveryIssue::InvalidControlLabel(name_issue(issue))),
        )),
    }
}
