//! Exact `Closure`, `IfLet`, and `Match` candidate graph validation.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::{AttachedCandidateExpressionChild, AttachedCandidateNode};
use arcweft_lang_syntax::expressions::{
    SyntaxExpressionSlot, SyntaxMatchBodyTerminator, SyntaxRequiredTokenState,
};

use super::{CandidateValidationCursor, source_index_has_typed_owner};
use crate::expr::{
    HirClosureExpr, HirExpressionRecoveryIssue, HirIfLetExpr, HirMatchExpr, HirMatchRecoveryIssue,
    HirPoisonState, HirRecoveryIssue,
};
use crate::identity::{ExprId, LocalGeneration, LocalId, ScopeId, SyntheticKey, SyntheticOwner};
use crate::scope::{HirPatternBindingPolicy, HirScopeKind, HirScopeOwner};
use crate::source_index::{HirExprSourceRole, HirMatchArmSourcePart, HirSourceSite};

// The outer `Option` aborts source freeze; the inner one is the control recovery payload.
#[allow(clippy::option_option)]
impl CandidateValidationCursor<'_> {
    pub(super) fn validate_closure(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        actual: &HirClosureExpr,
        outer_scope: ScopeId,
    ) -> Option<Option<HirRecoveryIssue>> {
        let attached = node.closure_view()?;
        let closure_scope = self.validate_scope(
            expression,
            outer_scope,
            HirScopeKind::Closure,
            &node.source_span(),
        )?;
        let mut generations = BTreeMap::<_, LocalGeneration>::new();
        let mut recovery = None;
        let mut locals = Vec::new();
        if actual.parameters().len() != attached.parameters().len() {
            return None;
        }
        for (source, retained) in attached.parameters().iter().zip(actual.parameters()) {
            let pattern = self.validate_pattern_binding(
                source.pattern(),
                closure_scope,
                HirPatternBindingPolicy::ClosureParameter,
                &mut generations,
            )?;
            if retained.pattern() != pattern.owner || retained.local_scope() != closure_scope {
                return None;
            }
            if let HirPoisonState::Poisoned(issue) = pattern.state {
                recovery.get_or_insert(issue);
            }
            let ty = match source.ty() {
                Some(ty) => Some(self.validate_type(ty, closure_scope)?),
                None => None,
            };
            if retained.ty() != ty.map(|ty| ty.id) {
                return None;
            }
            if let Some(ty) = ty.filter(|ty| ty.poisoned) {
                let payload = self.types.resolve_prepared(self.slots, ty.id).ok()?;
                let HirPoisonState::Poisoned(issue) = payload.state() else {
                    return None;
                };
                recovery.get_or_insert_with(|| issue.clone());
            }
            locals.extend(pattern.locals);
        }

        let result_type = match attached.result_type() {
            Some(ty) => Some(self.validate_type(ty, closure_scope)?),
            None => None,
        };
        if actual.result_type() != result_type.map(|ty| ty.id) {
            return None;
        }
        if let Some(ty) = result_type.filter(|ty| ty.poisoned) {
            let payload = self.types.resolve_prepared(self.slots, ty.id).ok()?;
            let HirPoisonState::Poisoned(issue) = payload.state() else {
                return None;
            };
            recovery.get_or_insert_with(|| issue.clone());
        }
        let body = match attached.body() {
            AttachedCandidateExpressionChild::Authored { node, .. }
            | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                let body = self.validate_expression(*node, closure_scope)?;
                if body.poisoned {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild {
                            role: HirExprSourceRole::Body,
                        },
                    ));
                }
                body.id
            }
            AttachedCandidateExpressionChild::Missing { source, .. } => {
                recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                self.validate_missing_tail(closure_scope, source)?
            }
        };
        self.finish_scope(closure_scope, &locals)?;
        if actual.scope() != closure_scope || actual.body() != body {
            return None;
        }
        Some(recovery)
    }

    pub(super) fn validate_if_let(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        actual: &HirIfLetExpr,
        outer_scope: ScopeId,
    ) -> Option<Option<HirRecoveryIssue>> {
        let attached = node.if_let_view()?;
        let binding_scope = self.validate_scope(
            expression,
            outer_scope,
            HirScopeKind::Conditional,
            &node.source_span(),
        )?;
        let mut generations = BTreeMap::<_, LocalGeneration>::new();
        let pattern = self.validate_pattern_binding(
            attached.pattern(),
            binding_scope,
            HirPatternBindingPolicy::PatternBinding,
            &mut generations,
        )?;
        let mut recovery = match pattern.state {
            HirPoisonState::Clean => None,
            HirPoisonState::Poisoned(issue) => Some(issue),
        };
        let scrutinee = self.validate_control_operand(
            attached.scrutinee(),
            outer_scope,
            HirExprSourceRole::Scrutinee,
            &mut recovery,
        )?;
        let guard = match attached.guard() {
            Some(guard) => Some(self.validate_control_operand(
                guard,
                binding_scope,
                HirExprSourceRole::Guard,
                &mut recovery,
            )?),
            None => None,
        };
        let then_branch = self.validate_control_operand(
            attached.then_branch(),
            binding_scope,
            HirExprSourceRole::ThenBranch,
            &mut recovery,
        )?;
        let else_branch = if let Some(else_branch) = attached.else_branch() {
            self.validate_control_operand(
                else_branch,
                outer_scope,
                HirExprSourceRole::ElseBranch,
                &mut recovery,
            )?
        } else {
            recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
            self.validate_missing_tail(outer_scope, attached.else_source_span())?
        };
        self.finish_scope(binding_scope, &pattern.locals)?;
        if actual.scope() != binding_scope
            || actual.pattern() != pattern.owner
            || actual.scrutinee() != scrutinee
            || actual.guard() != guard
            || actual.then_branch() != then_branch
            || actual.else_branch() != else_branch
        {
            return None;
        }
        Some(recovery)
    }

    // Arm scopes, bindings, guards, values, and recovery are one ordered contract.
    #[allow(clippy::too_many_lines)]
    pub(super) fn validate_match(
        &mut self,
        expression: ExprId,
        node: AttachedCandidateNode<'_>,
        actual: &HirMatchExpr,
        outer_scope: ScopeId,
    ) -> Option<Option<HirRecoveryIssue>> {
        let attached = node.match_view()?;
        let mut recovery = None;
        let scrutinee = self.validate_control_operand(
            attached.scrutinee(),
            outer_scope,
            HirExprSourceRole::Scrutinee,
            &mut recovery,
        )?;
        if actual.scrutinee() != scrutinee || actual.arms().len() != attached.arms().len() {
            return None;
        }
        for (source, retained) in attached.arms().iter().zip(actual.arms()) {
            let ordinal = source.ordinal();
            let arm_scope = self.validate_scope(
                expression,
                outer_scope,
                HirScopeKind::MatchArm,
                &source.node().source_span(),
            )?;
            let mut generations = BTreeMap::<_, LocalGeneration>::new();
            let pattern = self.validate_pattern_binding(
                source.pattern(),
                arm_scope,
                HirPatternBindingPolicy::MatchBinding,
                &mut generations,
            )?;
            if pattern.state.is_poisoned() {
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
            let guard = match (source.projection().guard(), source.guard()) {
                (None, None) => None,
                (Some(SyntaxExpressionSlot::Authored), Some(guard)) => Some(
                    self.validate_control_operand(guard, arm_scope, guard_role, &mut recovery)?,
                ),
                (Some(SyntaxExpressionSlot::Missing), Some(_)) => {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild { role: guard_role },
                    ));
                    None
                }
                _ => return None,
            };
            if !matches!(
                source.projection().arrow(),
                SyntaxRequiredTokenState::Present
            ) {
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
            let value = match source.value() {
                AttachedCandidateExpressionChild::Authored { node, .. }
                | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                    let value = self.validate_expression(*node, arm_scope)?;
                    if value.poisoned {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                            HirExpressionRecoveryIssue::RecoveredChild { role: value_role },
                        ));
                    }
                    value.id
                }
                AttachedCandidateExpressionChild::Missing { source, .. } => {
                    recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                    self.validate_missing_tail(arm_scope, source)?
                }
            };
            self.finish_scope(arm_scope, &pattern.locals)?;
            if retained.scope() != arm_scope
                || retained.pattern() != pattern.owner
                || retained.guard() != guard
                || retained.value() != value
                || retained.locals() != pattern.locals.as_ref()
            {
                return None;
            }
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
        Some(recovery)
    }

    fn validate_control_operand(
        &mut self,
        child: &AttachedCandidateExpressionChild<'_>,
        scope: ScopeId,
        role: HirExprSourceRole,
        recovery: &mut Option<HirRecoveryIssue>,
    ) -> Option<ExprId> {
        let value = match child {
            AttachedCandidateExpressionChild::Authored { node, .. }
            | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                self.validate_expression(*node, scope)?
            }
            AttachedCandidateExpressionChild::Missing { source, .. } => {
                recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
                return Some(self.validate_missing(role, source, scope)?.id);
            }
        };
        if value.poisoned {
            recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild { role },
            ));
        }
        Some(value.id)
    }

    pub(super) fn validate_missing_tail(
        &mut self,
        scope: ScopeId,
        source: &arcweft_source::SourceSpan,
    ) -> Option<ExprId> {
        let site = HirSourceSite::from_attached_span(self.parsed.document(), source).ok()?;
        let id = self.take_expression(&site, scope)?;
        let payload = self.expressions.resolve_prepared(self.slots, id).ok()?;
        matches!(
            payload.state(),
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
        )
        .then_some(())?;
        matches!(
            payload.kind(),
            crate::expr::HirExprKind::Error(error)
                if error.issue() == crate::expr::HirGenericExprIssue::TransactionalChildFailure
        )
        .then_some(id)
    }

    pub(super) fn validate_scope(
        &mut self,
        expression: ExprId,
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
        if metadata.origin() != &crate::slot::HirOrigin::Synthetic(key)
            || metadata.source_site() != &site
            || payload.kind() != kind
            || payload.parent() != Some(parent)
            || payload.owner() != &HirScopeOwner::Expr(expression)
            || source_index_has_typed_owner(self.index, SyntheticOwner::Scope(scope))
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

    pub(super) fn finish_scope(&self, scope: ScopeId, locals: &[LocalId]) -> Option<()> {
        let payload = self.scopes.resolve_prepared(self.slots, scope).ok()?;
        let children = self
            .expected
            .scope_children
            .get(&scope)
            .map_or(&[][..], Vec::as_slice);
        (payload.children() == children && payload.locals() == locals).then_some(())
    }
}
