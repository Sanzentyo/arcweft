//! Candidate-only Closure, IfLet, and Match lowering.

use arcweft_lang_syntax::attachment::{AttachedCandidateExpressionChild, AttachedCandidateNode};
use arcweft_lang_syntax::expressions::{
    SyntaxExpressionSlot, SyntaxMatchBodyTerminator, SyntaxRequiredTokenState,
};

use crate::expr::{
    HirClosureExpr, HirClosureParameter, HirExprKind, HirExpressionRecoveryIssue, HirIfLetExpr,
    HirMatchArm, HirMatchExpr, HirMatchRecoveryIssue, HirPoisonState, HirRecoveryIssue,
};
use crate::identity::{ExprId, LocalId, ScopeId, SyntheticKey, SyntheticOwner};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::{HirExprSourceRole, HirMatchArmSourcePart, HirSourceSite};

use super::CandidateCursor;
use crate::final_lowering::StagedHirModuleTransaction;

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_candidate_closure(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        outer_scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirClosureExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let attached = node
            .closure_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let closure_scope = self.allocate_candidate_expression_scope(
            expression,
            outer_scope,
            HirScopeKind::Closure,
            &node.source_span(),
            cursor,
        )?;
        let mut parameters = Vec::with_capacity(attached.parameters().len());
        let mut locals = Vec::new();
        let mut recovery = None;
        for parameter in attached.parameters() {
            let lowered = self.lower_candidate_pattern_binding(
                parameter.pattern(),
                closure_scope,
                HirPatternBindingPolicy::ClosureParameter,
                cursor,
            )?;
            if let HirPoisonState::Poisoned(issue) = self
                .arenas
                .patterns()
                .resolve_staged(&self.slots, lowered.owner)?
                .state()
            {
                recovery.get_or_insert_with(|| issue.clone());
            }
            let ty = parameter
                .ty()
                .map(|ty| self.lower_candidate_type(ty, closure_scope, cursor))
                .transpose()?;
            if let Some(ty) = ty {
                if let HirPoisonState::Poisoned(issue) =
                    self.arenas.types().resolve_staged(&self.slots, ty)?.state()
                {
                    recovery.get_or_insert_with(|| issue.clone());
                }
            }
            parameters.push(
                HirClosureParameter::try_new(lowered.owner, ty, closure_scope)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
            locals.extend(lowered.locals);
        }

        let result_type = attached
            .result_type()
            .map(|ty| self.lower_candidate_type(ty, closure_scope, cursor))
            .transpose()?;
        if let Some(result_type) = result_type {
            if let HirPoisonState::Poisoned(issue) = self
                .arenas
                .types()
                .resolve_staged(&self.slots, result_type)?
                .state()
            {
                recovery.get_or_insert_with(|| issue.clone());
            }
        }
        self.close_scope_members(closure_scope, locals.into_boxed_slice())?;

        self.begin_closure_captures(expression, closure_scope)?;
        let body = match attached.body() {
            AttachedCandidateExpressionChild::Authored { node, .. }
            | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                let body = self.lower_candidate_expression(*node, closure_scope, cursor)?;
                if self.staged_expression_is_poisoned(body)? {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild {
                            role: HirExprSourceRole::Body,
                        },
                    ));
                }
                body
            }
            AttachedCandidateExpressionChild::Missing { source, .. } => {
                recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                self.lower_missing_candidate_tail(closure_scope, cursor, source)?
            }
        };
        let captures = self.finish_closure_captures(expression)?;

        Ok((
            HirClosureExpr::new(
                closure_scope,
                parameters.into_boxed_slice(),
                result_type,
                body,
                captures,
            ),
            recovery,
        ))
    }

    pub(super) fn lower_candidate_if_let(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        outer_scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirIfLetExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let attached = node
            .if_let_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let binding_scope = self.allocate_candidate_expression_scope(
            expression,
            outer_scope,
            HirScopeKind::Conditional,
            &node.source_span(),
            cursor,
        )?;
        let lowered_pattern = self.lower_candidate_pattern_binding(
            attached.pattern(),
            binding_scope,
            HirPatternBindingPolicy::PatternBinding,
            cursor,
        )?;
        let mut recovery = match self
            .arenas
            .patterns()
            .resolve_staged(&self.slots, lowered_pattern.owner)?
            .state()
        {
            HirPoisonState::Clean => None,
            HirPoisonState::Poisoned(issue) => Some(issue.clone()),
        };
        let scrutinee = self.lower_candidate_control_operand(
            attached.scrutinee(),
            outer_scope,
            cursor,
            HirExprSourceRole::Scrutinee,
            &mut recovery,
        )?;
        let guard = attached
            .guard()
            .map(|guard| {
                self.lower_candidate_control_operand(
                    guard,
                    binding_scope,
                    cursor,
                    HirExprSourceRole::Guard,
                    &mut recovery,
                )
            })
            .transpose()?;
        let then_branch = self.lower_candidate_control_operand(
            attached.then_branch(),
            binding_scope,
            cursor,
            HirExprSourceRole::ThenBranch,
            &mut recovery,
        )?;
        let else_branch = match attached.else_branch() {
            Some(else_branch) => self.lower_candidate_control_operand(
                else_branch,
                outer_scope,
                cursor,
                HirExprSourceRole::ElseBranch,
                &mut recovery,
            )?,
            None => {
                recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                self.lower_missing_candidate_tail(outer_scope, cursor, attached.else_source_span())?
            }
        };
        self.close_scope_members(binding_scope, lowered_pattern.locals)?;
        Ok((
            HirIfLetExpr::new(
                binding_scope,
                lowered_pattern.owner,
                scrutinee,
                guard,
                then_branch,
                else_branch,
            ),
            recovery,
        ))
    }

    pub(super) fn lower_candidate_match(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        outer_scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(HirMatchExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let attached = node
            .match_view()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let mut recovery = None;
        let scrutinee = self.lower_candidate_control_operand(
            attached.scrutinee(),
            outer_scope,
            cursor,
            HirExprSourceRole::Scrutinee,
            &mut recovery,
        )?;
        let mut arms = Vec::with_capacity(attached.arms().len());
        for arm in attached.arms() {
            let ordinal = arm.ordinal();
            let arm_scope = self.allocate_candidate_expression_scope(
                expression,
                outer_scope,
                HirScopeKind::MatchArm,
                &arm.node().source_span(),
                cursor,
            )?;
            let lowered_pattern = self.lower_candidate_pattern_binding(
                arm.pattern(),
                arm_scope,
                HirPatternBindingPolicy::MatchBinding,
                cursor,
            )?;
            if self
                .arenas
                .patterns()
                .resolve_staged(&self.slots, lowered_pattern.owner)?
                .is_poisoned()
            {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: HirExprSourceRole::MatchArm {
                            arm: ordinal,
                            part: HirMatchArmSourcePart::Pattern,
                        },
                    },
                ));
            }
            let guard_role = HirExprSourceRole::MatchArm {
                arm: ordinal,
                part: HirMatchArmSourcePart::Guard,
            };
            let guard = match (arm.projection().guard(), arm.guard()) {
                (None, None) => None,
                (Some(SyntaxExpressionSlot::Authored), Some(guard)) => {
                    Some(self.lower_candidate_control_operand(
                        guard,
                        arm_scope,
                        cursor,
                        guard_role,
                        &mut recovery,
                    )?)
                }
                (Some(SyntaxExpressionSlot::Missing), Some(_)) => {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild { role: guard_role },
                    ));
                    None
                }
                _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
            };
            if !matches!(arm.projection().arrow(), SyntaxRequiredTokenState::Present) {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: HirExprSourceRole::MatchArm {
                            arm: ordinal,
                            part: HirMatchArmSourcePart::Arrow,
                        },
                    },
                ));
            }
            let value_role = HirExprSourceRole::MatchArm {
                arm: ordinal,
                part: HirMatchArmSourcePart::Value,
            };
            let value = match arm.value() {
                AttachedCandidateExpressionChild::Authored { node, .. }
                | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                    let value = self.lower_candidate_expression(*node, arm_scope, cursor)?;
                    if self.staged_expression_is_poisoned(value)? {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                            HirExpressionRecoveryIssue::RecoveredChild { role: value_role },
                        ));
                    }
                    value
                }
                AttachedCandidateExpressionChild::Missing { source, .. } => {
                    recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                    self.lower_missing_candidate_tail(arm_scope, cursor, source)?
                }
            };
            self.close_scope_members(arm_scope, lowered_pattern.locals.clone())?;
            arms.push(
                HirMatchArm::try_new(
                    arm_scope,
                    lowered_pattern.owner,
                    guard,
                    value,
                    lowered_pattern.locals,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }
        match attached.projection().terminator() {
            SyntaxMatchBodyTerminator::Closed => {}
            SyntaxMatchBodyTerminator::MissingBody => {
                recovery.get_or_insert(HirRecoveryIssue::InvalidMatch(
                    HirMatchRecoveryIssue::MissingBody,
                ));
            }
            SyntaxMatchBodyTerminator::RecoveredMissingClose => {
                recovery.get_or_insert(HirRecoveryIssue::InvalidMatch(
                    HirMatchRecoveryIssue::UnclosedBody,
                ));
            }
        }
        Ok((
            HirMatchExpr::try_new(scrutinee, arms.into_boxed_slice())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            recovery,
        ))
    }

    fn lower_candidate_control_operand(
        &mut self,
        child: &AttachedCandidateExpressionChild<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        role: HirExprSourceRole,
        recovery: &mut Option<HirRecoveryIssue>,
    ) -> Result<ExprId, HirLowerFailure> {
        let lowered = match child {
            AttachedCandidateExpressionChild::Authored { node, .. }
            | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                self.lower_candidate_expression(*node, scope, cursor)?
            }
            AttachedCandidateExpressionChild::Missing { source, .. } => {
                recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
                return self.lower_missing_candidate_expression(scope, cursor, role, source);
            }
        };
        if self.staged_expression_is_poisoned(lowered)? {
            recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild { role },
            ));
        }
        Ok(lowered)
    }

    pub(super) fn lower_missing_candidate_tail(
        &mut self,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        let ordinal = cursor.take_expression_ordinal()?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner()), cursor.role(), ordinal)
                .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let site = HirSourceSite::from_attached_span(self.request.source().document(), source)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, site.clone())?;
        let expression = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(expression, scope);
        }
        let payload = crate::expr::HirExpr::try_new(
            scope,
            HirExprKind::Error(crate::expr::HirExprError::new(
                crate::expr::HirGenericExprIssue::TransactionalChildFailure,
            )),
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.stage_candidate_recovery_diagnostic(SyntheticOwner::Expr(expression), site);
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    pub(super) fn allocate_candidate_expression_scope(
        &mut self,
        expression: ExprId,
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
                expression.module(),
                kind,
                Some(parent),
                HirScopeOwner::Expr(expression),
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
            && retained.owner() == &HirScopeOwner::Expr(expression)
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }
}
