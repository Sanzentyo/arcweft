//! Assignment-family source-freeze evidence.

use arcweft_lang_syntax::attachment::node::{AssignmentStatementKind, LifetimeSetStatementKind};
use arcweft_lang_syntax::attachment::{RequiredStatementExpressionNode, StatementNode};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::arena::ArenaSnapshot;
use crate::expr::HirExpr;
use crate::identity::{ExprId, ScopeId, StmtId};
use crate::slot::SlotSnapshot;
use crate::source_index::HirExprSourceRole;
use crate::stmt::{HirStmtChildRole, HirStmtPoisonState, HirStmtRecoveryIssue};

use super::{StatementEvidence, missing_statement_expression_matches, source_expression_matches};

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
    let (attached_target, attached_value) = match attached.kind() {
        SyntaxKind::AssignmentStatement => {
            let attached = attached.cast::<AssignmentStatementKind>().ok()?;
            (attached.target().ok()?, attached.value().ok()?)
        }
        SyntaxKind::LifetimeSetStatement => {
            let attached = attached.cast::<LifetimeSetStatementKind>().ok()?;
            (attached.target().ok()?, attached.value().ok()?)
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
        0,
        HirExprSourceRole::Target,
    ) || !assignment_expression_matches(
        parsed,
        slots,
        expressions,
        statement,
        value,
        attached_value,
        scope,
        1,
        HirExprSourceRole::Operand,
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
    ordinal: u32,
    role: HirExprSourceRole,
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
            missing.range().start(),
            ordinal,
            role,
        ),
    }
}
