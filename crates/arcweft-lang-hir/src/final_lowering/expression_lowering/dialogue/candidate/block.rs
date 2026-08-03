//! Candidate-only value-block and ordinary statement lowering.
//!
//! Candidate graph nodes intentionally have no `SyntaxNodeId`. Every HIR
//! descendant is allocated from the outer E34 expression owner and one
//! interpretation role; no source-backed reader or source-text reparse is
//! available on this path.

mod keyword;
mod required_operand;

use arcweft_lang_syntax::attachment::{
    AttachedCandidateBlockTail, AttachedCandidateIfElse, AttachedCandidateIfHead,
    AttachedCandidateMatchArmBody, AttachedCandidateMatchBody, AttachedCandidateNode,
    AttachedCandidateStatement, AttachedCandidateStatementBlock,
    AttachedCandidateStatementExpression, AttachedCandidateUnsafeAuditId,
    AttachedCandidateUnsafeBody,
};
use arcweft_lang_syntax::expressions::{ExpressionProjection, SyntaxComputationBlockKind};
use arcweft_lang_syntax::grammar::{SyntaxKind, SyntaxRole};
use arcweft_lang_syntax::name::SyntaxNameIssue;

use crate::diagnostic::{HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::expr::{
    HirBlockExpr, HirComputationBlockExpr, HirComputationBlockKind, HirExpressionRecoveryIssue,
    HirNamedBlockExpr, HirNamedBlockName, HirRecoveryIssue,
};
use crate::identity::{ExprId, HirLimit, LocalId, ScopeId, StmtId, SyntheticKey, SyntheticOwner};
use crate::leaf::{HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::{HirExprSourceRole, HirMatchArmSourcePart, HirSourceSite};
use crate::stmt::{
    HirAssertionMode, HirConditionalElseBranch, HirContextualStmtBody, HirIfLetStmt, HirIfStmt,
    HirMatchStmt, HirStmt, HirStmtChildRole, HirStmtKind, HirStmtMatchArm, HirStmtMatchArmBody,
    HirStmtPoisonState, HirStmtRecoveryIssue, HirThreadStmtBodyRole, HirThreadStmtRecoveryIssue,
    HirUnsafeAudit, HirUnsafeLifetimeBody,
};

use super::CandidateCursor;
use crate::final_lowering::StagedHirModuleTransaction;
use crate::final_lowering::name_projection::{name, name_issue, require_attempted_name_limit};
use crate::final_lowering::require_limit;

#[derive(Clone, Copy)]
enum CandidateOmittedTail {
    ImplicitUnit,
    MissingRequired,
}

struct LoweredCandidateValueBlock {
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
    recovery: Option<HirRecoveryIssue>,
}

struct LoweredCandidateStatement {
    owner: StmtId,
    locals: Box<[LocalId]>,
    poisoned: bool,
}

struct LoweredCandidateStatementBlock {
    body: Box<[StmtId]>,
    locals: Box<[LocalId]>,
    poisoned: bool,
    first_poisoned: Option<u32>,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_candidate_block(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        parent_scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirBlockExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let lowered = self.lower_candidate_value_block(
            expression,
            node,
            parent_scope,
            cursor,
            CandidateOmittedTail::ImplicitUnit,
        )?;
        Ok((
            HirBlockExpr::new(lowered.scope, lowered.statements, lowered.tail),
            lowered.recovery,
        ))
    }

    pub(super) fn lower_candidate_computation_block(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        parent_scope: ScopeId,
        cursor: &mut CandidateCursor,
        source_kind: SyntaxComputationBlockKind,
    ) -> Result<(HirComputationBlockExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let (kind, omitted_tail) = match source_kind {
            SyntaxComputationBlockKind::Result => (
                HirComputationBlockKind::Result,
                CandidateOmittedTail::MissingRequired,
            ),
            SyntaxComputationBlockKind::Task => (
                HirComputationBlockKind::Task,
                CandidateOmittedTail::MissingRequired,
            ),
            SyntaxComputationBlockKind::Seq => (
                HirComputationBlockKind::Seq,
                CandidateOmittedTail::ImplicitUnit,
            ),
            SyntaxComputationBlockKind::Stream => (
                HirComputationBlockKind::Stream,
                CandidateOmittedTail::ImplicitUnit,
            ),
        };
        let lowered =
            self.lower_candidate_value_block(expression, node, parent_scope, cursor, omitted_tail)?;
        Ok((
            HirComputationBlockExpr::new(kind, lowered.scope, lowered.statements, lowered.tail),
            lowered.recovery,
        ))
    }

    pub(super) fn lower_candidate_named_block(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        parent_scope: ScopeId,
        cursor: &mut CandidateCursor,
        source_name: &Result<arcweft_lang_syntax::name::SyntaxName, SyntaxNameIssue>,
    ) -> Result<(HirNamedBlockExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let (block_name, name_recovery) = match source_name {
            Ok(source_name) => (HirNamedBlockName::Resolved(name(source_name)?), None),
            Err(SyntaxNameIssue::Missing) => {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            Err(issue) => {
                require_attempted_name_limit(issue)?;
                let issue = name_issue(issue);
                (
                    HirNamedBlockName::InvalidPresent(issue),
                    Some(HirRecoveryIssue::InvalidName(issue)),
                )
            }
        };
        let lowered = self.lower_candidate_value_block(
            expression,
            node,
            parent_scope,
            cursor,
            CandidateOmittedTail::ImplicitUnit,
        )?;
        Ok((
            HirNamedBlockExpr::new(block_name, lowered.scope, lowered.statements, lowered.tail),
            name_recovery.or(lowered.recovery),
        ))
    }

    fn lower_candidate_value_block(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        parent_scope: ScopeId,
        cursor: &mut CandidateCursor,
        omitted_tail: CandidateOmittedTail,
    ) -> Result<LoweredCandidateValueBlock, HirLowerFailure> {
        let block = node
            .value_block_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        require_limit(HirLimit::Statements, block.statements().len())?;
        let scope = self.allocate_candidate_expression_scope(
            expression,
            parent_scope,
            HirScopeKind::Block,
            &block.block().source_span(),
            cursor,
        )?;
        let mut statement_ids = Vec::with_capacity(block.statements().len());
        let mut locals = Vec::new();
        let mut recovery = None;
        for statement in block.statements() {
            let lowered = self.lower_candidate_statement(*statement, scope, cursor)?;
            if lowered.poisoned {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: HirExprSourceRole::Statement {
                            ordinal: statement.ordinal(),
                        },
                    },
                ));
            }
            statement_ids.push(lowered.owner);
            locals.extend(lowered.locals);
        }
        require_limit(HirLimit::LocalsPerScope, locals.len())?;
        self.close_scope_members(scope, locals.into_boxed_slice())?;

        let tail = match block.tail() {
            AttachedCandidateBlockTail::Expression(tail) => {
                let tail = self.lower_candidate_statement_expression(
                    tail,
                    scope,
                    cursor,
                    HirExprSourceRole::Tail,
                )?;
                if self.staged_expression_is_poisoned(tail)? {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild {
                            role: HirExprSourceRole::Tail,
                        },
                    ));
                }
                tail
            }
            AttachedCandidateBlockTail::Omitted { node } => match omitted_tail {
                CandidateOmittedTail::ImplicitUnit => {
                    self.lower_candidate_implicit_unit(scope, cursor, &node.source_span())?
                }
                CandidateOmittedTail::MissingRequired => {
                    recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                    self.lower_missing_candidate_tail(scope, cursor, &node.source_span())?
                }
            },
        };

        Ok(LoweredCandidateValueBlock {
            scope,
            statements: statement_ids.into_boxed_slice(),
            tail,
            recovery,
        })
    }

    fn lower_candidate_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<LoweredCandidateStatement, HirLowerFailure> {
        let ordinal = cursor.take_statement_ordinal()?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner()), cursor.role(), ordinal)
                .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let site = HirSourceSite::from_attached_span(
            self.request.source().document(),
            &statement.source_span(),
        )
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation =
            self.arenas
                .statements()
                .reserve_synthetic(&mut self.slots, key, site.clone())?;
        let owner = reservation.id();
        if !reservation.is_first_touch() {
            let retained = self
                .arenas
                .statements()
                .resolve_staged(&self.slots, owner)?;
            if retained.scope() != scope {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            return Ok(LoweredCandidateStatement {
                owner,
                locals: retained.kind().post_statement_locals().into(),
                poisoned: retained.is_poisoned(),
            });
        }

        let (kind, locals, recovery) = match statement.kind() {
            SyntaxKind::AssertionStatement => {
                let assertion = statement
                    .assertion_view()
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                require_limit(HirLimit::AssertionConditions, assertion.conditions().len())?;
                let mut condition_recovery = false;
                let mut conditions = Vec::with_capacity(assertion.conditions().len());
                for condition in assertion.conditions() {
                    let condition = self.lower_candidate_statement_expression(
                        *condition,
                        scope,
                        cursor,
                        HirExprSourceRole::Condition,
                    )?;
                    condition_recovery |= self.staged_expression_is_poisoned(condition)?;
                    conditions.push(condition);
                }
                let mode = assertion
                    .mode()
                    .map_or(HirAssertionMode::Recovered, HirAssertionMode::Resolved);
                let recovery = if matches!(mode, HirAssertionMode::Recovered) {
                    Some(HirStmtRecoveryIssue::InvalidAssertionMode)
                } else if conditions.is_empty() {
                    Some(HirStmtRecoveryIssue::MissingAssertionCondition)
                } else if condition_recovery {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Condition,
                    })
                } else if assertion.has_recovery() {
                    Some(HirStmtRecoveryIssue::MalformedAssertion)
                } else {
                    None
                };
                (
                    HirStmtKind::Assertion {
                        mode,
                        conditions: conditions.into_boxed_slice(),
                    },
                    Box::<[LocalId]>::from([]),
                    recovery,
                )
            }
            statement_kind @ (SyntaxKind::AssignmentStatement
            | SyntaxKind::LifetimeSetStatement) => {
                let assignment = statement
                    .assignment_view()
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let target = self.lower_candidate_statement_expression(
                    assignment.target(),
                    scope,
                    cursor,
                    HirExprSourceRole::Target,
                )?;
                if statement_kind == SyntaxKind::AssignmentStatement {
                    self.upgrade_direct_reassignment_capture(target)?;
                }
                let target_poisoned = self.staged_expression_is_poisoned(target)?;
                let value = self.lower_candidate_statement_expression(
                    assignment.value(),
                    scope,
                    cursor,
                    HirExprSourceRole::Operand,
                )?;
                let value_poisoned = self.staged_expression_is_poisoned(value)?;
                let kind = if statement_kind == SyntaxKind::AssignmentStatement {
                    HirStmtKind::Assign { target, value }
                } else {
                    HirStmtKind::LifetimeSet { target, value }
                };
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
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::ReturnStatement
            | SyntaxKind::YieldStatement
            | SyntaxKind::WaitStatement
            | SyntaxKind::CloseStatement
            | SyntaxKind::SelectStatement => {
                let (kind, recovery) =
                    self.lower_candidate_required_operand_statement(statement, scope, cursor)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::LetStatement => {
                let initializer = statement
                    .required_expression(SyntaxRole::Initializer)
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let initializer = self.lower_candidate_statement_expression(
                    initializer,
                    scope,
                    cursor,
                    HirExprSourceRole::Operand,
                )?;
                let initializer_poisoned = self.staged_expression_is_poisoned(initializer)?;
                let pattern = statement
                    .required_pattern(SyntaxRole::Pattern)
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let lowered = self.lower_candidate_pattern_binding(
                    pattern,
                    scope,
                    HirPatternBindingPolicy::LetBinding,
                    cursor,
                )?;
                let locals = lowered.locals;
                (
                    HirStmtKind::Let {
                        pattern: lowered.owner,
                        annotation: None,
                        initializer,
                        locals: locals.clone(),
                    },
                    locals,
                    if lowered.poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Pattern,
                        })
                    } else if initializer_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Initializer,
                        })
                    } else {
                        None
                    },
                )
            }
            SyntaxKind::ExpressionStatement => {
                let expression = statement
                    .required_expression(SyntaxRole::Initializer)
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let expression = self.lower_candidate_statement_expression(
                    expression,
                    scope,
                    cursor,
                    HirExprSourceRole::Operand,
                )?;
                let poisoned = self.staged_expression_is_poisoned(expression)?;
                (
                    HirStmtKind::Expression { expression },
                    Box::<[LocalId]>::from([]),
                    poisoned.then_some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Expression,
                    }),
                )
            }
            SyntaxKind::OutStatement
            | SyntaxKind::GotoStatement
            | SyntaxKind::DeferStatement
            | SyntaxKind::SignalStatement
            | SyntaxKind::BreakStatement
            | SyntaxKind::ContinueStatement => {
                let (kind, recovery) =
                    self.lower_candidate_keyword_statement(statement, scope, cursor)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::ProofCallStatement => {
                let call = statement
                    .required_expression(SyntaxRole::Callee)
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let call = self.lower_candidate_statement_expression(
                    call,
                    scope,
                    cursor,
                    HirExprSourceRole::Operand,
                )?;
                let poisoned = self.staged_expression_is_poisoned(call)?;
                (
                    HirStmtKind::ProofCall { call },
                    Box::<[LocalId]>::from([]),
                    poisoned.then_some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Expression,
                    }),
                )
            }
            SyntaxKind::IfStatement => {
                let (kind, recovery) =
                    self.lower_candidate_if_statement(statement, owner, scope, cursor)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::MatchStatement => {
                let (kind, recovery) =
                    self.lower_candidate_match_statement(statement, owner, scope, cursor)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::UnsafeLifetimeStatement => {
                let (kind, recovery) = self
                    .lower_candidate_unsafe_lifetime_statement(statement, owner, scope, cursor)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::ErrorStatement => (
                HirStmtKind::Error,
                Box::<[LocalId]>::from([]),
                Some(HirStmtRecoveryIssue::UnclassifiedSyntax),
            ),
            _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
        };
        let state = recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned);
        let payload = HirStmt::try_new_with_state(scope, kind, state)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        if payload.is_poisoned() {
            self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
                SyntheticOwner::Stmt(owner),
                HirRecoveryPrimary::owner_whole(SyntheticOwner::Stmt(owner)),
                site,
            ));
        }
        let poisoned = payload.is_poisoned();
        self.arenas
            .statements()
            .finalize(&mut self.slots, reservation, payload)?;
        Ok(LoweredCandidateStatement {
            owner,
            locals,
            poisoned,
        })
    }

    fn lower_candidate_if_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        owner: StmtId,
        outer_scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let source = statement
            .if_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        match source.head() {
            AttachedCandidateIfHead::Condition(condition) => {
                let condition = self.lower_candidate_statement_expression(
                    *condition,
                    outer_scope,
                    cursor,
                    HirExprSourceRole::Condition,
                )?;
                let condition_poisoned = self.staged_expression_is_poisoned(condition)?;
                let then_scope = self.allocate_candidate_statement_scope(
                    owner,
                    outer_scope,
                    HirScopeKind::Conditional,
                    &source.then_branch().node().source_span(),
                    cursor,
                )?;
                let then_branch =
                    self.lower_candidate_statement_block(source.then_branch(), then_scope, cursor)?;
                self.close_scope_members(then_scope, then_branch.locals)?;

                let then_poisoned = then_branch.poisoned;
                let then_body = HirContextualStmtBody::try_ordinary(then_scope, then_branch.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (else_branch, else_poisoned) = match source.else_branch() {
                    None => (None, false),
                    Some(AttachedCandidateIfElse::Block(block)) => {
                        let branch_scope = self.allocate_candidate_statement_scope(
                            owner,
                            outer_scope,
                            HirScopeKind::Conditional,
                            &block.node().source_span(),
                            cursor,
                        )?;
                        let branch =
                            self.lower_candidate_statement_block(block, branch_scope, cursor)?;
                        self.close_scope_members(branch_scope, branch.locals)?;
                        let branch_poisoned = branch.poisoned;
                        let body = HirContextualStmtBody::try_ordinary(branch_scope, branch.body)
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        (Some(HirConditionalElseBranch::body(body)), branch_poisoned)
                    }
                    Some(AttachedCandidateIfElse::If(_)) => {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                };
                let statement = HirIfStmt::try_new(condition, then_body, else_branch)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok((
                    HirStmtKind::If(statement),
                    if condition_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Condition,
                        })
                    } else if then_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::ThenBranch,
                        })
                    } else if else_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::ElseBranch,
                        })
                    } else {
                        None
                    },
                ))
            }
            AttachedCandidateIfHead::Let {
                pattern,
                scrutinee,
                guard,
            } => {
                let scrutinee = self.lower_candidate_statement_expression(
                    *scrutinee,
                    outer_scope,
                    cursor,
                    HirExprSourceRole::Scrutinee,
                )?;
                let scrutinee_poisoned = self.staged_expression_is_poisoned(scrutinee)?;
                let then_scope = self.allocate_candidate_statement_scope(
                    owner,
                    outer_scope,
                    HirScopeKind::Conditional,
                    &source.then_branch().node().source_span(),
                    cursor,
                )?;
                let lowered_pattern = self.lower_candidate_pattern_binding(
                    *pattern,
                    then_scope,
                    HirPatternBindingPolicy::PatternBinding,
                    cursor,
                )?;
                let pattern_poisoned = lowered_pattern.poisoned;
                let pattern_locals = lowered_pattern.locals;
                let guard = guard
                    .map(|guard| {
                        self.lower_candidate_statement_expression(
                            guard,
                            then_scope,
                            cursor,
                            HirExprSourceRole::Guard,
                        )
                    })
                    .transpose()?;
                let guard_poisoned = guard
                    .map(|guard| self.staged_expression_is_poisoned(guard))
                    .transpose()?
                    .unwrap_or(false);
                let then_branch =
                    self.lower_candidate_statement_block(source.then_branch(), then_scope, cursor)?;
                let then_poisoned = then_branch.poisoned;
                let mut then_locals = pattern_locals.to_vec();
                then_locals.extend(then_branch.locals);
                require_limit(HirLimit::LocalsPerScope, then_locals.len())?;
                self.close_scope_members(then_scope, then_locals.into_boxed_slice())?;
                let then_body = HirContextualStmtBody::try_ordinary(then_scope, then_branch.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

                let (else_branch, else_poisoned) = match source.else_branch() {
                    None => (None, false),
                    Some(AttachedCandidateIfElse::Block(block)) => {
                        let branch_scope = self.allocate_candidate_statement_scope(
                            owner,
                            outer_scope,
                            HirScopeKind::Conditional,
                            &block.node().source_span(),
                            cursor,
                        )?;
                        let branch =
                            self.lower_candidate_statement_block(block, branch_scope, cursor)?;
                        self.close_scope_members(branch_scope, branch.locals)?;
                        let branch_poisoned = branch.poisoned;
                        let body = HirContextualStmtBody::try_ordinary(branch_scope, branch.body)
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        (Some(HirConditionalElseBranch::body(body)), branch_poisoned)
                    }
                    Some(AttachedCandidateIfElse::If(nested)) => {
                        let nested =
                            self.lower_candidate_statement(*nested, outer_scope, cursor)?;
                        (
                            Some(HirConditionalElseBranch::else_if(nested.owner)),
                            nested.poisoned,
                        )
                    }
                };
                let if_let = HirIfLetStmt::try_new(
                    lowered_pattern.owner,
                    scrutinee,
                    guard,
                    then_body,
                    pattern_locals,
                    else_branch,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok((
                    HirStmtKind::IfLet(if_let),
                    if pattern_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Pattern,
                        })
                    } else if scrutinee_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Scrutinee,
                        })
                    } else if guard_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Guard,
                        })
                    } else if then_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::ThenBranch,
                        })
                    } else if else_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::ElseBranch,
                        })
                    } else {
                        None
                    },
                ))
            }
        }
    }

    fn lower_candidate_match_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        owner: StmtId,
        outer_scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let source = statement
            .match_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let scrutinee = self.lower_candidate_statement_expression(
            source.scrutinee(),
            outer_scope,
            cursor,
            HirExprSourceRole::Scrutinee,
        )?;
        let mut recovery = self.staged_expression_is_poisoned(scrutinee)?.then_some(
            HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Scrutinee,
            },
        );
        let (source_arms, body_unclosed) = match source.body() {
            AttachedCandidateMatchBody::Missing { .. } => {
                recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                    HirThreadStmtRecoveryIssue::MissingBody {
                        role: HirThreadStmtBodyRole::Match,
                    },
                ));
                (&[][..], false)
            }
            AttachedCandidateMatchBody::Block { arms, close, .. } => {
                (arms.as_ref(), close.source_span().range().is_empty())
            }
        };
        let mut arms = Vec::with_capacity(source_arms.len());
        for source_arm in source_arms {
            let arm_ordinal = source_arm.ordinal();
            let arm_scope = self.allocate_candidate_statement_scope(
                owner,
                outer_scope,
                HirScopeKind::MatchArm,
                &source_arm.node().source_span(),
                cursor,
            )?;
            let lowered_pattern = self.lower_candidate_pattern_binding(
                source_arm.pattern(),
                arm_scope,
                HirPatternBindingPolicy::MatchBinding,
                cursor,
            )?;
            if lowered_pattern.poisoned {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmPattern { arm: arm_ordinal },
                });
            }
            let guard = source_arm
                .guard()
                .map(|guard| {
                    self.lower_candidate_statement_expression(
                        guard,
                        arm_scope,
                        cursor,
                        HirExprSourceRole::Guard,
                    )
                })
                .transpose()?;
            if guard
                .map(|guard| self.staged_expression_is_poisoned(guard))
                .transpose()?
                .unwrap_or(false)
            {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmGuard { arm: arm_ordinal },
                });
            }

            let pattern_locals = lowered_pattern.locals;
            let mut scope_locals = pattern_locals.to_vec();
            let body = match source_arm.body() {
                AttachedCandidateMatchArmBody::Expression(
                    AttachedCandidateStatementExpression::Missing(node),
                ) => {
                    let expression =
                        self.lower_missing_candidate_tail(arm_scope, cursor, &node.source_span())?;
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                    });
                    HirStmtMatchArmBody::Expression(expression)
                }
                AttachedCandidateMatchArmBody::Expression(expression) => {
                    let expression = self.lower_candidate_statement_expression(
                        *expression,
                        arm_scope,
                        cursor,
                        HirExprSourceRole::MatchArm {
                            arm: arm_ordinal,
                            part: HirMatchArmSourcePart::Value,
                        },
                    )?;
                    if self.staged_expression_is_poisoned(expression)? {
                        recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                        });
                    }
                    HirStmtMatchArmBody::Expression(expression)
                }
                AttachedCandidateMatchArmBody::Block(block) => {
                    let lowered = self.lower_candidate_statement_block(block, arm_scope, cursor)?;
                    scope_locals.extend(lowered.locals);
                    if let Some(statement) = lowered.first_poisoned {
                        recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::MatchArmBodyStatement {
                                arm: arm_ordinal,
                                statement,
                            },
                        });
                    }
                    let body = HirContextualStmtBody::try_ordinary(arm_scope, lowered.body)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    HirStmtMatchArmBody::Body(body)
                }
            };
            require_limit(HirLimit::LocalsPerScope, scope_locals.len())?;
            self.close_scope_members(arm_scope, scope_locals.into_boxed_slice())?;
            arms.push(
                HirStmtMatchArm::try_new(
                    arm_scope,
                    lowered_pattern.owner,
                    guard,
                    body,
                    pattern_locals,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }
        if source_arms.is_empty()
            && !matches!(source.body(), AttachedCandidateMatchBody::Missing { .. })
        {
            recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::EmptyMatch,
            ));
        }
        if body_unclosed {
            recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::UnclosedBody {
                    role: HirThreadStmtBodyRole::Match,
                },
            ));
        }
        let statement = HirMatchStmt::try_new(scrutinee, arms.into_boxed_slice())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((HirStmtKind::Match(statement), recovery))
    }

    fn lower_candidate_unsafe_lifetime_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        owner: StmtId,
        outer_scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let source = statement
            .unsafe_lifetime_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let audit_id = match source.audit_id() {
            AttachedCandidateUnsafeAuditId::Reference(node) => {
                let Some(ExpressionProjection::EntityReference(reference)) =
                    node.expression_projection()
                else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                crate::final_lowering::id_ref_projection::id_ref(reference)?
            }
            AttachedCandidateUnsafeAuditId::Missing(_) => HirIdRefValue::Recovered(
                HirIdRefRecovery::new(HirIdRefShape::Missing, HirIdRefIssue::Missing),
            ),
        };
        let mut recovery = audit_id
            .recovery_issue()
            .map(HirStmtRecoveryIssue::InvalidAuditId);
        let reason = source
            .reason()
            .map(|reason| {
                self.lower_candidate_statement_expression(
                    reason,
                    outer_scope,
                    cursor,
                    HirExprSourceRole::Operand,
                )
            })
            .transpose()?;
        if reason
            .map(|reason| self.staged_expression_is_poisoned(reason))
            .transpose()?
            .unwrap_or(false)
        {
            recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Reason,
            });
        }

        let (body, has_safety_doc) = match source.body() {
            AttachedCandidateUnsafeBody::Missing(_) => {
                recovery.get_or_insert(HirStmtRecoveryIssue::MissingBody);
                (HirUnsafeLifetimeBody::Missing, false)
            }
            AttachedCandidateUnsafeBody::Block(block) => {
                let body_scope = self.allocate_candidate_statement_scope(
                    owner,
                    outer_scope,
                    HirScopeKind::Block,
                    &block.node().source_span(),
                    cursor,
                )?;
                let lowered = self.lower_candidate_statement_block(block, body_scope, cursor)?;
                self.close_scope_members(body_scope, lowered.locals)?;
                if let Some(ordinal) = lowered.first_poisoned {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::BodyStatement { ordinal },
                    });
                }
                if !block.is_closed() {
                    recovery.get_or_insert(HirStmtRecoveryIssue::UnclosedBody);
                }
                (
                    HirUnsafeLifetimeBody::Block {
                        scope: body_scope,
                        statements: lowered.body,
                    },
                    !block.safety_documentation().is_empty(),
                )
            }
        };
        Ok((
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(audit_id, reason, has_safety_doc),
                body,
            },
            recovery,
        ))
    }

    fn lower_candidate_statement_block(
        &mut self,
        block: &AttachedCandidateStatementBlock<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<LoweredCandidateStatementBlock, HirLowerFailure> {
        require_limit(HirLimit::Statements, block.statements().len())?;
        let mut body = Vec::with_capacity(block.statements().len());
        let mut locals = Vec::new();
        let mut poisoned = false;
        let mut first_poisoned = None;
        for (ordinal, statement) in block.statements().iter().enumerate() {
            let lowered = self.lower_candidate_statement(*statement, scope, cursor)?;
            body.push(lowered.owner);
            locals.extend(lowered.locals);
            poisoned |= lowered.poisoned;
            if lowered.poisoned && first_poisoned.is_none() {
                first_poisoned = Some(
                    u32::try_from(ordinal).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                );
            }
        }
        require_limit(HirLimit::LocalsPerScope, locals.len())?;
        Ok(LoweredCandidateStatementBlock {
            body: body.into_boxed_slice(),
            locals: locals.into_boxed_slice(),
            poisoned,
            first_poisoned,
        })
    }

    fn allocate_candidate_statement_scope(
        &mut self,
        owner: StmtId,
        parent: ScopeId,
        kind: HirScopeKind,
        source: &arcweft_source::SourceSpan,
        cursor: &mut CandidateCursor,
    ) -> Result<ScopeId, HirLowerFailure> {
        let ordinal = cursor.take_scope_ordinal()?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner()), cursor.role(), ordinal)
                .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let site = HirSourceSite::from_attached_span(self.request.source().document(), source)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation = self
            .arenas
            .scopes()
            .reserve_synthetic(&mut self.slots, key, site)?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                owner.module(),
                kind,
                Some(parent),
                HirScopeOwner::Stmt(owner),
                Box::new([]),
                Box::<[LocalId]>::from([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .finalize(&mut self.slots, reservation, payload)?;
            self.append_scope_child(parent, scope)?;
            return Ok(scope);
        }
        let retained = self.arenas.scopes().resolve_staged(&self.slots, scope)?;
        if retained.kind() == kind
            && retained.parent() == Some(parent)
            && retained.owner() == &HirScopeOwner::Stmt(owner)
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }

    fn lower_candidate_statement_expression(
        &mut self,
        expression: AttachedCandidateStatementExpression<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        missing_role: HirExprSourceRole,
    ) -> Result<ExprId, HirLowerFailure> {
        match expression {
            AttachedCandidateStatementExpression::Authored(node)
            | AttachedCandidateStatementExpression::Recovered(node) => {
                self.lower_candidate_expression(node, scope, cursor)
            }
            AttachedCandidateStatementExpression::Missing(node) => self
                .lower_missing_candidate_expression(
                    scope,
                    cursor,
                    missing_role,
                    &node.source_span(),
                ),
        }
    }
}
