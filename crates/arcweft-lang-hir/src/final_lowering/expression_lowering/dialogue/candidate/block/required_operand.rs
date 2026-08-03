//! Candidate-local required-operand statement lowering.

use arcweft_lang_syntax::attachment::AttachedCandidateStatement;
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::final_lowering::StagedHirModuleTransaction;
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::HirExprSourceRole;
use crate::stmt::{HirSelectStmt, HirStmtChildRole, HirStmtKind, HirStmtRecoveryIssue};

use super::super::CandidateCursor;

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

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_candidate_required_operand_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        scope: crate::identity::ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let family = match statement.kind() {
            SyntaxKind::ReturnStatement => RequiredOperandFamily::Return,
            SyntaxKind::YieldStatement => RequiredOperandFamily::Yield,
            SyntaxKind::WaitStatement => RequiredOperandFamily::Wait,
            SyntaxKind::CloseStatement => RequiredOperandFamily::Close,
            SyntaxKind::SelectStatement => RequiredOperandFamily::Select,
            _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
        };
        let source = statement
            .required_operand_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let expression = self.lower_candidate_statement_expression(
            source.operand(),
            scope,
            cursor,
            family.source_role(),
        )?;
        let expression_poisoned = self.staged_expression_is_poisoned(expression)?;
        let recovery = if expression_poisoned {
            Some(HirStmtRecoveryIssue::RecoveredChild {
                role: family.recovery_role(),
            })
        } else if source.has_punctuation_recovery() {
            Some(HirStmtRecoveryIssue::MalformedWait)
        } else {
            None
        };
        let kind = match family {
            RequiredOperandFamily::Return => HirStmtKind::Return { value: expression },
            RequiredOperandFamily::Yield => HirStmtKind::Yield { expression },
            RequiredOperandFamily::Wait => HirStmtKind::Wait { target: expression },
            RequiredOperandFamily::Close => HirStmtKind::Close { target: expression },
            RequiredOperandFamily::Select => {
                HirStmtKind::Select(HirSelectStmt::operand(expression))
            }
        };
        Ok((kind, recovery))
    }
}
