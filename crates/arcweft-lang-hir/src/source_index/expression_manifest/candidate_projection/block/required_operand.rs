//! Candidate required-operand statement source-freeze evidence.

use arcweft_lang_syntax::attachment::AttachedCandidateStatement;
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::identity::ScopeId;
use crate::source_index::HirExprSourceRole;
use crate::stmt::{
    HirSelectStmt, HirStmtChildRole, HirStmtKind, HirStmtPoisonState, HirStmtRecoveryIssue,
};

use super::super::CandidateValidationCursor;

#[derive(Clone, Copy)]
enum RequiredOperandFamily {
    Return,
    Yield,
    Wait,
    Close,
    Select,
}

impl RequiredOperandFamily {
    const fn source_role(self) -> HirExprSourceRole {
        match self {
            Self::Wait | Self::Close => HirExprSourceRole::Target,
            Self::Return | Self::Yield | Self::Select => HirExprSourceRole::Operand,
        }
    }

    const fn recovery_role(self) -> HirStmtChildRole {
        match self {
            Self::Wait | Self::Close => HirStmtChildRole::Target,
            Self::Return | Self::Yield | Self::Select => HirStmtChildRole::Expression,
        }
    }
}

impl CandidateValidationCursor<'_> {
    pub(super) fn validate_required_operand_statement(
        &mut self,
        source: AttachedCandidateStatement<'_>,
        scope: ScopeId,
        payload: &HirStmtKind,
    ) -> Option<(HirStmtPoisonState, bool)> {
        let family = match source.kind() {
            SyntaxKind::ReturnStatement => RequiredOperandFamily::Return,
            SyntaxKind::YieldStatement => RequiredOperandFamily::Yield,
            SyntaxKind::WaitStatement => RequiredOperandFamily::Wait,
            SyntaxKind::CloseStatement => RequiredOperandFamily::Close,
            SyntaxKind::SelectStatement => RequiredOperandFamily::Select,
            _ => return None,
        };
        let source = source.required_operand_view()?;
        let expression =
            self.validate_statement_expression(source.operand(), scope, family.source_role())?;
        let recovery = if expression.poisoned {
            Some(HirStmtRecoveryIssue::RecoveredChild {
                role: family.recovery_role(),
            })
        } else if source.has_punctuation_recovery() {
            Some(HirStmtRecoveryIssue::MalformedWait)
        } else {
            None
        };
        let payload_matches = match family {
            RequiredOperandFamily::Return => {
                matches!(payload, HirStmtKind::Return { value } if *value == expression.id)
            }
            RequiredOperandFamily::Yield => {
                matches!(payload, HirStmtKind::Yield { expression: actual } if *actual == expression.id)
            }
            RequiredOperandFamily::Wait => {
                matches!(payload, HirStmtKind::Wait { target } if *target == expression.id)
            }
            RequiredOperandFamily::Close => {
                matches!(payload, HirStmtKind::Close { target } if *target == expression.id)
            }
            RequiredOperandFamily::Select => {
                matches!(payload, HirStmtKind::Select(HirSelectStmt::Operand(actual)) if *actual == expression.id)
            }
        };
        Some((
            recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
            payload_matches,
        ))
    }
}
