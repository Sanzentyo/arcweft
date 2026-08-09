//! Assignment-family source-freeze evidence.

use arcweft_lang_syntax::attachment::node::{AssignmentStatementKind, LifetimeSetStatementKind};
use arcweft_lang_syntax::attachment::{RequiredStatementExpressionNode, StatementNode};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::arena::ArenaSnapshot;
use crate::expr::HirExpr;
use crate::identity::{ExprId, ScopeId, StmtId};
use crate::slot::SlotSnapshot;
use crate::source_index::HirStmtRecoveryOperandSlot;
use crate::stmt::{HirStmtChildRole, HirStmtPoisonState, HirStmtRecoveryIssue};

use super::{StatementEvidence, missing_statement_expression_matches, source_expression_matches};

#[derive(Clone, Copy)]
enum AssignmentFamily {
    Assignment,
    LifetimeSet,
}

impl AssignmentFamily {
    const fn target_slot(self, insertion: usize) -> HirStmtRecoveryOperandSlot {
        match self {
            Self::Assignment => HirStmtRecoveryOperandSlot::AssignmentTarget { insertion },
            Self::LifetimeSet => HirStmtRecoveryOperandSlot::LifetimeSetTarget { insertion },
        }
    }

    const fn value_slot(self, insertion: usize) -> HirStmtRecoveryOperandSlot {
        match self {
            Self::Assignment => HirStmtRecoveryOperandSlot::AssignmentValue { insertion },
            Self::LifetimeSet => HirStmtRecoveryOperandSlot::LifetimeSetValue { insertion },
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assignment_statement_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statement: StmtId,
    attached: &StatementNode,
    target: ExprId,
    value: ExprId,
    scope: ScopeId,
) -> Option<StatementEvidence> {
    let (family, attached_target, attached_value) = match attached.kind() {
        SyntaxKind::AssignmentStatement => {
            let attached = attached.cast::<AssignmentStatementKind>().ok()?;
            (
                AssignmentFamily::Assignment,
                attached.target().ok()?,
                attached.value().ok()?,
            )
        }
        SyntaxKind::LifetimeSetStatement => {
            let attached = attached.cast::<LifetimeSetStatementKind>().ok()?;
            (
                AssignmentFamily::LifetimeSet,
                attached.target().ok()?,
                attached.value().ok()?,
            )
        }
        _ => return None,
    };
    if !assignment_expression_matches(
        parsed,
        slots,
        expressions,
        statement,
        target,
        attached_target,
        scope,
        family,
        true,
    ) || !assignment_expression_matches(
        parsed,
        slots,
        expressions,
        statement,
        value,
        attached_value,
        scope,
        family,
        false,
    ) {
        return None;
    }
    let state = if expressions
        .resolve_prepared(slots, target)
        .is_ok_and(HirExpr::is_poisoned)
    {
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Target,
        })
    } else if expressions
        .resolve_prepared(slots, value)
        .is_ok_and(HirExpr::is_poisoned)
    {
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Initializer,
        })
    } else {
        HirStmtPoisonState::Clean
    };
    Some(StatementEvidence {
        locals: Box::new([]),
        state,
    })
}

#[allow(clippy::too_many_arguments)]
fn assignment_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statement: StmtId,
    owner: ExprId,
    attached: RequiredStatementExpressionNode,
    scope: ScopeId,
    family: AssignmentFamily,
    target: bool,
) -> bool {
    match attached {
        RequiredStatementExpressionNode::Expression(attached) => {
            attached.semantic().is_ok_and(|attached| {
                source_expression_matches(slots, expressions, owner, &attached, scope)
            })
        }
        RequiredStatementExpressionNode::Missing(missing) => {
            let slot = if target {
                family.target_slot(missing.range().start())
            } else {
                family.value_slot(missing.range().start())
            };
            missing_statement_expression_matches(
                parsed,
                slots,
                expressions,
                statement,
                owner,
                scope,
                slot,
            )
        }
    }
}
