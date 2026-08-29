//! Exact source freeze for candidate value blocks and their admitted statements.

mod keyword;
mod required_operand;

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::{
    AttachedCandidateBlockTail, AttachedCandidateIfElse, AttachedCandidateIfHead,
    AttachedCandidateMatchArmBody, AttachedCandidateMatchBody, AttachedCandidateNode,
    AttachedCandidateStatement, AttachedCandidateStatementBlock,
    AttachedCandidateStatementExpression, AttachedCandidateUnsafeAuditId,
    AttachedCandidateUnsafeBody,
};
use arcweft_lang_syntax::expressions::ExpressionProjection;
use arcweft_lang_syntax::grammar::{SyntaxKind, SyntaxRole};

use super::{CandidateChild, CandidateValidationCursor};
use crate::expr::{HirExpressionRecoveryIssue, HirRecoveryIssue};
use crate::identity::{
    ExprId, LocalGeneration, LocalId, ScopeId, StmtId, SyntheticKey, SyntheticOwner,
};
use crate::leaf::HirName;
use crate::scope::{HirPatternBindingPolicy, HirScopeKind, HirScopeOwner};
use crate::slot::HirOrigin;
use crate::source_index::{HirExprSourceRole, HirMatchArmSourcePart, HirSourceSite};
use crate::stmt::{
    HirAssertionMode, HirConditionalElseBranch, HirContextualStmtBody, HirStmtChildRole,
    HirStmtKind, HirStmtMatchArmBody, HirStmtPoisonState, HirStmtRecoveryIssue,
    HirThreadStmtBodyRole, HirThreadStmtRecoveryIssue, HirUnsafeAuditIdentity,
    HirUnsafeAuditIdentityIssue, HirUnsafeLifetimeBody,
};

#[derive(Clone, Copy)]
pub(super) enum CandidateTailPolicy {
    ImplicitUnit,
    MissingRequired,
}

impl CandidateValidationCursor<'_> {
    #[allow(clippy::too_many_arguments, clippy::option_option)]
    pub(super) fn validate_value_block(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        parent_scope: ScopeId,
        actual_scope: ScopeId,
        actual_statements: &[StmtId],
        actual_tail: ExprId,
        tail_policy: CandidateTailPolicy,
    ) -> Option<Option<HirRecoveryIssue>> {
        let source = node.value_block_view()?;
        let scope = self.validate_scope(
            expression,
            parent_scope,
            HirScopeKind::Block,
            &source.block().source_span(),
        )?;
        if scope != actual_scope || source.statements().len() != actual_statements.len() {
            return None;
        }

        let mut recovery = None;
        let mut locals = Vec::new();
        let mut generations = BTreeMap::<HirName, LocalGeneration>::new();
        for (statement, retained) in source.statements().iter().zip(actual_statements) {
            let lowered = self.validate_statement(*statement, scope, &mut generations)?;
            if lowered.owner != *retained {
                return None;
            }
            if lowered.poisoned {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: HirExprSourceRole::Statement {
                            ordinal: statement.ordinal(),
                        },
                    },
                ));
            }
            locals.extend(lowered.locals);
        }
        self.finish_scope(scope, &locals)?;

        let tail = match source.tail() {
            AttachedCandidateBlockTail::Expression(source) => {
                let tail =
                    self.validate_statement_expression(source, scope, HirExprSourceRole::Tail)?;
                if tail.poisoned {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild {
                            role: HirExprSourceRole::Tail,
                        },
                    ));
                }
                tail.id
            }
            AttachedCandidateBlockTail::Omitted { node } => match tail_policy {
                CandidateTailPolicy::ImplicitUnit => {
                    self.validate_implicit_unit(&node.source_span(), scope)?
                }
                CandidateTailPolicy::MissingRequired => {
                    recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                    self.validate_missing_tail(scope, &node.source_span())?
                }
            },
        };
        (tail == actual_tail).then_some(recovery)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive candidate-statement dispatcher validates the closed typed family"
    )]
    fn validate_statement(
        &mut self,
        source: AttachedCandidateStatement<'_>,
        scope: ScopeId,
        generations: &mut BTreeMap<HirName, LocalGeneration>,
    ) -> Option<CandidateStatement> {
        let owner = self.take_statement(source, scope)?;
        let payload = self.statements.resolve_prepared(self.slots, owner).ok()?;
        let (locals, expected_state, payload_matches) = match source.kind() {
            SyntaxKind::AssertionStatement => {
                let assertion = source.assertion_view()?;
                let mut condition_recovery = false;
                let mut conditions = Vec::with_capacity(assertion.conditions().len());
                for condition in assertion.conditions() {
                    let condition = self.validate_statement_expression(
                        *condition,
                        scope,
                        HirExprSourceRole::Condition,
                    )?;
                    condition_recovery |= condition.poisoned;
                    conditions.push(condition.id);
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
                    Box::<[LocalId]>::from([]),
                    recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    matches!(
                        payload.kind(),
                        HirStmtKind::Assertion {
                            mode: actual_mode,
                            conditions: actual_conditions,
                        } if actual_mode == &mode && actual_conditions.as_ref() == conditions
                    ),
                )
            }
            statement_kind @ (SyntaxKind::AssignmentStatement
            | SyntaxKind::LifetimeSetStatement) => {
                let assignment = source.assignment_view()?;
                let target = self.validate_statement_expression(
                    assignment.target(),
                    scope,
                    HirExprSourceRole::Target,
                )?;
                let value = self.validate_statement_expression(
                    assignment.value(),
                    scope,
                    HirExprSourceRole::Operand,
                )?;
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
                let payload_matches = if statement_kind == SyntaxKind::AssignmentStatement {
                    matches!(
                        payload.kind(),
                        HirStmtKind::Assign {
                            target: actual_target,
                            value: actual_value,
                        } if *actual_target == target.id && *actual_value == value.id
                    )
                } else {
                    matches!(
                        payload.kind(),
                        HirStmtKind::LifetimeSet {
                            target: actual_target,
                            value: actual_value,
                        } if *actual_target == target.id && *actual_value == value.id
                    )
                };
                (
                    Box::<[LocalId]>::from([]),
                    recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    payload_matches,
                )
            }
            SyntaxKind::ReturnStatement
            | SyntaxKind::YieldStatement
            | SyntaxKind::WaitStatement
            | SyntaxKind::CloseStatement
            | SyntaxKind::SelectStatement => {
                let (state, matches) =
                    self.validate_required_operand_statement(source, scope, payload.kind())?;
                (Box::<[LocalId]>::from([]), state, matches)
            }
            SyntaxKind::LetStatement => {
                let initializer = self.validate_statement_expression(
                    source.required_expression(SyntaxRole::Initializer)?,
                    scope,
                    HirExprSourceRole::Operand,
                )?;
                let pattern = self.validate_pattern_binding(
                    source.required_pattern(SyntaxRole::Pattern)?,
                    scope,
                    HirPatternBindingPolicy::LetBinding,
                    generations,
                )?;
                let recovery = if pattern.state.is_poisoned() {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Pattern,
                    })
                } else if initializer.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    })
                } else {
                    None
                };
                let matches = matches!(
                    payload.kind(),
                    HirStmtKind::Let {
                        pattern: actual_pattern,
                        annotation: None,
                        initializer: actual_initializer,
                        locals,
                    } if *actual_pattern == pattern.owner
                        && *actual_initializer == initializer.id
                        && locals.as_ref() == pattern.locals.as_ref()
                );
                (
                    pattern.locals,
                    recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    matches,
                )
            }
            SyntaxKind::LetElseStatement => {
                let source = source.let_else_view()?;
                let initializer = self.validate_statement_expression(
                    source.initializer(),
                    scope,
                    HirExprSourceRole::Operand,
                )?;
                let generations_before_pattern = generations.clone();
                let pattern = self.validate_pattern_binding(
                    source.pattern(),
                    scope,
                    HirPatternBindingPolicy::LetElseBinding,
                    generations,
                )?;
                let else_scope = self.validate_statement_scope(
                    owner,
                    scope,
                    HirScopeKind::Block,
                    &source.else_branch().node().source_span(),
                )?;
                let mut else_generations = generations_before_pattern;
                let else_branch = self.validate_statement_block(
                    source.else_branch(),
                    else_scope,
                    &mut else_generations,
                )?;
                self.finish_scope(else_scope, &else_branch.locals)?;
                let recovery = if pattern.state.is_poisoned() {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Pattern,
                    })
                } else if initializer.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    })
                } else if else_branch.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::ElseBranch,
                    })
                } else {
                    None
                };
                let matches = matches!(
                    payload.kind(),
                    HirStmtKind::LetElse {
                        pattern: actual_pattern,
                        annotation: None,
                        initializer: actual_initializer,
                        else_scope: actual_else_scope,
                        else_body,
                        locals,
                    } if *actual_pattern == pattern.owner
                        && *actual_initializer == initializer.id
                        && *actual_else_scope == else_scope
                        && else_body.as_ref() == else_branch.body.as_ref()
                        && locals.as_ref() == pattern.locals.as_ref()
                );
                (
                    pattern.locals,
                    recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    matches,
                )
            }
            SyntaxKind::ExpressionStatement => {
                let expression = self.validate_statement_expression(
                    source.required_expression(SyntaxRole::Initializer)?,
                    scope,
                    HirExprSourceRole::Operand,
                )?;
                (
                    Box::<[LocalId]>::from([]),
                    expression
                        .poisoned
                        .then_some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Expression,
                        })
                        .map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    matches!(payload.kind(), HirStmtKind::Expression { expression: actual }
                        if *actual == expression.id),
                )
            }
            SyntaxKind::OutStatement
            | SyntaxKind::GotoStatement
            | SyntaxKind::DeferStatement
            | SyntaxKind::SignalStatement
            | SyntaxKind::BreakStatement
            | SyntaxKind::ContinueStatement => {
                let (state, matches) =
                    self.validate_keyword_statement(source, scope, payload.kind())?;
                (Box::<[LocalId]>::from([]), state, matches)
            }
            SyntaxKind::ProofCallStatement => {
                let call = self.validate_statement_expression(
                    source.required_expression(SyntaxRole::Callee)?,
                    scope,
                    HirExprSourceRole::Operand,
                )?;
                (
                    Box::<[LocalId]>::from([]),
                    call.poisoned
                        .then_some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Expression,
                        })
                        .map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    matches!(payload.kind(), HirStmtKind::ProofCall { call: actual }
                        if *actual == call.id),
                )
            }
            SyntaxKind::IfStatement => {
                let expected = self.validate_if_statement(source, owner, scope)?;
                (
                    Box::<[LocalId]>::from([]),
                    expected
                        .recovery
                        .map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    expected.matches,
                )
            }
            SyntaxKind::MatchStatement => {
                let expected = self.validate_match_statement(source, owner, scope)?;
                (
                    Box::<[LocalId]>::from([]),
                    expected
                        .recovery
                        .map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    expected.matches,
                )
            }
            SyntaxKind::UnsafeLifetimeStatement => {
                let expected = self.validate_unsafe_lifetime_statement(source, owner, scope)?;
                (
                    Box::<[LocalId]>::from([]),
                    expected
                        .recovery
                        .map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
                    expected.matches,
                )
            }
            SyntaxKind::ErrorStatement => (
                Box::<[LocalId]>::from([]),
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::UnclassifiedSyntax),
                matches!(payload.kind(), HirStmtKind::Error),
            ),
            _ => return None,
        };
        (payload_matches && payload.state() == &expected_state).then_some(CandidateStatement {
            owner,
            locals,
            poisoned: payload.is_poisoned(),
        })
    }

    fn take_statement(
        &mut self,
        source: AttachedCandidateStatement<'_>,
        scope: ScopeId,
    ) -> Option<StmtId> {
        let ordinal = self.next_statement;
        self.next_statement = ordinal.checked_add(1)?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(self.outer), self.role, ordinal).ok()?;
        let owner = self
            .slots
            .resolve_prepared_synthetic::<StmtId>(key)
            .ok()??;
        let metadata = self.slots.resolve_prepared(owner).ok()?;
        let payload = self.statements.resolve_prepared(self.slots, owner).ok()?;
        let site = HirSourceSite::from_attached_span(self.parsed.document(), &source.source_span())
            .ok()?;
        if metadata.origin() != &HirOrigin::Synthetic(key)
            || metadata.source_site() != &site
            || payload.scope() != scope
            || self.source_index_has_typed_owner(SyntheticOwner::Stmt(owner))
            || !self.expected.statements.insert(owner)
        {
            return None;
        }
        Some(owner)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one candidate If projection proves branches, bodies, source roles, and recovery together"
    )]
    fn validate_if_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        owner: StmtId,
        outer_scope: ScopeId,
    ) -> Option<CandidateStatementExpectation> {
        let source = statement.if_view()?;
        let payload = self.statements.resolve_prepared(self.slots, owner).ok()?;
        match source.head() {
            AttachedCandidateIfHead::Condition(condition) => {
                let condition = self.validate_statement_expression(
                    *condition,
                    outer_scope,
                    HirExprSourceRole::Condition,
                )?;
                let then_scope = self.validate_statement_scope(
                    owner,
                    outer_scope,
                    HirScopeKind::Conditional,
                    &source.then_branch().node().source_span(),
                )?;
                let mut then_generations = BTreeMap::new();
                let then_branch = self.validate_statement_block(
                    source.then_branch(),
                    then_scope,
                    &mut then_generations,
                )?;
                self.finish_scope(then_scope, &then_branch.locals)?;
                let then_body =
                    HirContextualStmtBody::try_ordinary(then_scope, then_branch.body.clone())
                        .ok()?;
                let (else_branch, else_poisoned) = match source.else_branch() {
                    None => (None, false),
                    Some(AttachedCandidateIfElse::Block(block)) => {
                        let branch_scope = self.validate_statement_scope(
                            owner,
                            outer_scope,
                            HirScopeKind::Conditional,
                            &block.node().source_span(),
                        )?;
                        let mut generations = BTreeMap::new();
                        let branch =
                            self.validate_statement_block(block, branch_scope, &mut generations)?;
                        self.finish_scope(branch_scope, &branch.locals)?;
                        let body =
                            HirContextualStmtBody::try_ordinary(branch_scope, branch.body).ok()?;
                        (Some(HirConditionalElseBranch::body(body)), branch.poisoned)
                    }
                    Some(AttachedCandidateIfElse::If(_)) => return None,
                };
                let recovery = if condition.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Condition,
                    })
                } else if then_branch.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::ThenBranch,
                    })
                } else if else_poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::ElseBranch,
                    })
                } else {
                    None
                };
                Some(CandidateStatementExpectation {
                    recovery,
                    matches: matches!(payload.kind(), HirStmtKind::If(actual)
                        if actual.condition() == condition.id
                            && actual.then_body() == &then_body
                            && actual.else_branch() == else_branch.as_ref()),
                })
            }
            AttachedCandidateIfHead::Let {
                pattern,
                scrutinee,
                guard,
            } => {
                let scrutinee = self.validate_statement_expression(
                    *scrutinee,
                    outer_scope,
                    HirExprSourceRole::Scrutinee,
                )?;
                let then_scope = self.validate_statement_scope(
                    owner,
                    outer_scope,
                    HirScopeKind::Conditional,
                    &source.then_branch().node().source_span(),
                )?;
                let mut then_generations = BTreeMap::new();
                let pattern = self.validate_pattern_binding(
                    *pattern,
                    then_scope,
                    HirPatternBindingPolicy::PatternBinding,
                    &mut then_generations,
                )?;
                let guard = match guard {
                    Some(guard) => Some(self.validate_statement_expression(
                        *guard,
                        then_scope,
                        HirExprSourceRole::Guard,
                    )?),
                    None => None,
                };
                let then_branch = self.validate_statement_block(
                    source.then_branch(),
                    then_scope,
                    &mut then_generations,
                )?;
                let mut then_locals = pattern.locals.to_vec();
                then_locals.extend_from_slice(&then_branch.locals);
                self.finish_scope(then_scope, &then_locals)?;

                let (else_branch, else_poisoned) = match source.else_branch() {
                    None => (None, false),
                    Some(AttachedCandidateIfElse::Block(block)) => {
                        let branch_scope = self.validate_statement_scope(
                            owner,
                            outer_scope,
                            HirScopeKind::Conditional,
                            &block.node().source_span(),
                        )?;
                        let mut generations = BTreeMap::new();
                        let branch =
                            self.validate_statement_block(block, branch_scope, &mut generations)?;
                        self.finish_scope(branch_scope, &branch.locals)?;
                        let body =
                            HirContextualStmtBody::try_ordinary(branch_scope, branch.body).ok()?;
                        (Some(HirConditionalElseBranch::body(body)), branch.poisoned)
                    }
                    Some(AttachedCandidateIfElse::If(nested)) => {
                        let mut generations = BTreeMap::new();
                        let nested =
                            self.validate_statement(*nested, outer_scope, &mut generations)?;
                        (
                            Some(HirConditionalElseBranch::ElseIf(nested.owner)),
                            nested.poisoned,
                        )
                    }
                };
                let guard_poisoned = guard.is_some_and(|guard| guard.poisoned);
                let recovery = if pattern.state.is_poisoned() {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Pattern,
                    })
                } else if scrutinee.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Scrutinee,
                    })
                } else if guard_poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Guard,
                    })
                } else if then_branch.poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::ThenBranch,
                    })
                } else if else_poisoned {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::ElseBranch,
                    })
                } else {
                    None
                };
                let matches = match payload.kind() {
                    HirStmtKind::IfLet(actual) => {
                        let then_body =
                            HirContextualStmtBody::try_ordinary(then_scope, then_branch.body)
                                .ok()?;
                        actual.pattern() == pattern.owner
                            && actual.scrutinee() == scrutinee.id
                            && actual.guard() == guard.map(|guard| guard.id)
                            && actual.then_body() == &then_body
                            && actual.locals() == pattern.locals.as_ref()
                            && actual.else_branch() == else_branch.as_ref()
                    }
                    _ => false,
                };
                Some(CandidateStatementExpectation { recovery, matches })
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one candidate Match projection proves scrutinee, arm scopes, bodies, and recovery together"
    )]
    fn validate_match_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        owner: StmtId,
        outer_scope: ScopeId,
    ) -> Option<CandidateStatementExpectation> {
        let source = statement.match_view()?;
        let payload = self.statements.resolve_prepared(self.slots, owner).ok()?;
        let HirStmtKind::Match(actual) = payload.kind() else {
            return Some(CandidateStatementExpectation {
                recovery: None,
                matches: false,
            });
        };
        let scrutinee = self.validate_statement_expression(
            source.scrutinee(),
            outer_scope,
            HirExprSourceRole::Scrutinee,
        )?;
        let mut recovery = scrutinee
            .poisoned
            .then_some(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Scrutinee,
            });
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
        if source_arms.len() != actual.arms().len() || actual.scrutinee() != scrutinee.id {
            return Some(CandidateStatementExpectation {
                recovery,
                matches: false,
            });
        }
        for (source_arm, actual_arm) in source_arms.iter().zip(actual.arms()) {
            let arm_ordinal = source_arm.ordinal();
            let arm_scope = self.validate_statement_scope(
                owner,
                outer_scope,
                HirScopeKind::MatchArm,
                &source_arm.node().source_span(),
            )?;
            let mut generations = BTreeMap::new();
            let pattern = self.validate_pattern_binding(
                source_arm.pattern(),
                arm_scope,
                HirPatternBindingPolicy::MatchBinding,
                &mut generations,
            )?;
            if pattern.state.is_poisoned() {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmPattern { arm: arm_ordinal },
                });
            }
            let guard = match source_arm.guard() {
                Some(guard) => Some(self.validate_statement_expression(
                    guard,
                    arm_scope,
                    HirExprSourceRole::Guard,
                )?),
                None => None,
            };
            if guard.is_some_and(|guard| guard.poisoned) {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmGuard { arm: arm_ordinal },
                });
            }
            let mut scope_locals = pattern.locals.to_vec();
            let body = match source_arm.body() {
                AttachedCandidateMatchArmBody::Expression(
                    AttachedCandidateStatementExpression::Missing(node),
                ) => {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                    });
                    HirStmtMatchArmBody::Expression(
                        self.validate_missing_tail(arm_scope, &node.source_span())?,
                    )
                }
                AttachedCandidateMatchArmBody::Expression(expression) => {
                    let expression = self.validate_statement_expression(
                        *expression,
                        arm_scope,
                        HirExprSourceRole::MatchArm {
                            arm: arm_ordinal,
                            part: HirMatchArmSourcePart::Value,
                        },
                    )?;
                    if expression.poisoned {
                        recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                        });
                    }
                    HirStmtMatchArmBody::Expression(expression.id)
                }
                AttachedCandidateMatchArmBody::Block(block) => {
                    let body = self.validate_statement_block(block, arm_scope, &mut generations)?;
                    scope_locals.extend_from_slice(&body.locals);
                    if let Some(statement) = body.first_poisoned {
                        recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::MatchArmBodyStatement {
                                arm: arm_ordinal,
                                statement,
                            },
                        });
                    }
                    let body = HirContextualStmtBody::try_ordinary(arm_scope, body.body).ok()?;
                    HirStmtMatchArmBody::Body(body)
                }
            };
            self.finish_scope(arm_scope, &scope_locals)?;
            if actual_arm.scope() != arm_scope
                || actual_arm.pattern() != pattern.owner
                || actual_arm.guard() != guard.map(|guard| guard.id)
                || actual_arm.body() != &body
                || actual_arm.locals() != pattern.locals.as_ref()
            {
                return Some(CandidateStatementExpectation {
                    recovery,
                    matches: false,
                });
            }
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
        Some(CandidateStatementExpectation {
            recovery,
            matches: true,
        })
    }

    fn validate_unsafe_lifetime_statement(
        &mut self,
        statement: AttachedCandidateStatement<'_>,
        owner: StmtId,
        outer_scope: ScopeId,
    ) -> Option<CandidateStatementExpectation> {
        let source = statement.unsafe_lifetime_view()?;
        let payload = self.statements.resolve_prepared(self.slots, owner).ok()?;
        let HirStmtKind::UnsafeLifetime {
            audit: actual_audit,
            body: actual_body,
        } = payload.kind()
        else {
            return Some(CandidateStatementExpectation {
                recovery: None,
                matches: false,
            });
        };
        let audit_identity = match source.audit_id() {
            AttachedCandidateUnsafeAuditId::Reference(node) => {
                let Some(ExpressionProjection::EntityReference(reference)) =
                    node.expression_projection()
                else {
                    return None;
                };
                crate::final_lowering::id_ref_projection::unsafe_audit_identity(reference).ok()?
            }
            AttachedCandidateUnsafeAuditId::Missing(_) => {
                HirUnsafeAuditIdentity::Recovered(HirUnsafeAuditIdentityIssue::Missing)
            }
        };
        let mut recovery = audit_identity
            .recovery_issue()
            .map(HirStmtRecoveryIssue::InvalidAuditId);
        let reason = match source.reason() {
            Some(reason) => Some(self.validate_statement_expression(
                reason,
                outer_scope,
                HirExprSourceRole::Operand,
            )?),
            None => None,
        };
        if reason.is_some_and(|reason| reason.poisoned) {
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
                let body_scope = self.validate_statement_scope(
                    owner,
                    outer_scope,
                    HirScopeKind::Block,
                    &block.node().source_span(),
                )?;
                let mut generations = BTreeMap::new();
                let body = self.validate_statement_block(block, body_scope, &mut generations)?;
                self.finish_scope(body_scope, &body.locals)?;
                if let Some(ordinal) = body.first_poisoned {
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
                        statements: body.body,
                    },
                    !block.safety_documentation().is_empty(),
                )
            }
        };
        Some(CandidateStatementExpectation {
            recovery,
            matches: actual_audit.identity() == &audit_identity
                && actual_audit.reason() == reason.map(|reason| reason.id)
                && actual_audit.has_safety_doc() == has_safety_doc
                && actual_body == &body,
        })
    }

    fn validate_statement_block(
        &mut self,
        block: &AttachedCandidateStatementBlock<'_>,
        scope: ScopeId,
        generations: &mut BTreeMap<HirName, LocalGeneration>,
    ) -> Option<CandidateStatementBlock> {
        let mut body = Vec::with_capacity(block.statements().len());
        let mut locals = Vec::new();
        let mut poisoned = false;
        let mut first_poisoned = None;
        for (ordinal, statement) in block.statements().iter().enumerate() {
            let statement = self.validate_statement(*statement, scope, generations)?;
            body.push(statement.owner);
            locals.extend(statement.locals);
            poisoned |= statement.poisoned;
            if statement.poisoned && first_poisoned.is_none() {
                first_poisoned = Some(u32::try_from(ordinal).ok()?);
            }
        }
        Some(CandidateStatementBlock {
            body: body.into_boxed_slice(),
            locals: locals.into_boxed_slice(),
            poisoned,
            first_poisoned,
        })
    }

    fn validate_statement_scope(
        &mut self,
        owner: StmtId,
        parent: ScopeId,
        kind: HirScopeKind,
        source: &arcweft_source::SourceSpan,
    ) -> Option<ScopeId> {
        let ordinal = self.next_scope;
        self.next_scope = ordinal.checked_add(1)?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(self.outer), self.role, ordinal).ok()?;
        let scope = self
            .slots
            .resolve_prepared_synthetic::<ScopeId>(key)
            .ok()??;
        let metadata = self.slots.resolve_prepared(scope).ok()?;
        let payload = self.scopes.resolve_prepared(self.slots, scope).ok()?;
        let site = HirSourceSite::from_attached_span(self.parsed.document(), source).ok()?;
        if metadata.origin() != &HirOrigin::Synthetic(key)
            || metadata.source_site() != &site
            || payload.kind() != kind
            || payload.parent() != Some(parent)
            || payload.owner() != &HirScopeOwner::Stmt(owner)
            || self.source_index_has_typed_owner(SyntheticOwner::Scope(scope))
            || !self.expected.scopes.insert(scope)
        {
            return None;
        }
        self.expected
            .scope_children
            .entry(parent)
            .or_default()
            .push(scope);
        self.expected.scope_children.entry(scope).or_default();
        Some(scope)
    }

    fn validate_statement_expression(
        &mut self,
        source: AttachedCandidateStatementExpression<'_>,
        scope: ScopeId,
        missing_role: HirExprSourceRole,
    ) -> Option<CandidateChild> {
        match source {
            AttachedCandidateStatementExpression::Authored(node)
            | AttachedCandidateStatementExpression::Recovered(node) => {
                self.validate_expression(node, scope)
            }
            AttachedCandidateStatementExpression::Missing(node) => {
                self.validate_missing(missing_role, &node.source_span(), scope)
            }
        }
    }
}

struct CandidateStatement {
    owner: StmtId,
    locals: Box<[LocalId]>,
    poisoned: bool,
}

struct CandidateStatementBlock {
    body: Box<[StmtId]>,
    locals: Box<[LocalId]>,
    poisoned: bool,
    first_poisoned: Option<u32>,
}

struct CandidateStatementExpectation {
    recovery: Option<HirStmtRecoveryIssue>,
    matches: bool,
}
