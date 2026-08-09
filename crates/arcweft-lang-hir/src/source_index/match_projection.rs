//! Cross-arena freeze validation for final-HIR Match expressions.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxExpressionSlot, SyntaxMatchBodyTerminator,
    SyntaxRequiredTokenState,
};
use arcweft_lang_syntax::incremental::ParsedSource;

use super::block_projection::{
    BlockValidationArenas, missing_scope_tail_matches, source_expression_matches,
    source_owner_matches,
};
use super::control_projection::canonical_pattern_locals;
use super::expression_manifest::projection::{expression_child_matches, poison_state_matches};
use super::pattern_projection::{BindingLocalValidation, binding_locals_match};
use super::{HirExprSourceRole, HirMatchArmSourcePart, HirSourceSite};
use crate::expr::{
    HirExpr, HirExpressionRecoveryIssue, HirMatchExpr, HirMatchRecoveryIssue, HirRecoveryIssue,
};
use crate::identity::ExprId;
use crate::scope::{HirLocalKind, HirPatternBindingPolicy, HirScopeKind, HirScopeOwner};
use crate::slot::SlotSnapshot;

/// Re-derives the complete E32 arm/scope/binding graph from the accepted
/// attached owner. Match arms deliberately have no HIR arena identity of
/// their own; their distinct source-backed scopes are the synthetic-tail and
/// local-visibility owners.
#[allow(
    clippy::too_many_lines,
    reason = "one projection proves Match scrutinee, per-arm scopes, bindings, values, source roles, and recovery together"
)]
pub(super) fn match_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ExprId,
    payload: &HirExpr,
    expression: &HirMatchExpr,
    attached: &arcweft_lang_syntax::attachment::AttachedExpressionNode,
) -> bool {
    let ExpressionProjection::Match(projection) = attached.projection() else {
        return false;
    };
    if projection.arms().len() != expression.arms().len()
        || attached.match_arms().len() != expression.arms().len()
        || attached.children().len() != 1
    {
        return false;
    }

    let scrutinee_child = &attached.children()[0];
    if scrutinee_child.component_role() != ExpressionComponentRole::Scrutinee
        || scrutinee_child.authored().is_some()
            != matches!(projection.scrutinee(), SyntaxExpressionSlot::Authored)
        || !expression_child_matches(
            parsed,
            slots,
            arenas.expressions,
            owner,
            payload.scope(),
            attached,
            scrutinee_child,
            expression.scrutinee(),
        )
    {
        return false;
    }
    let mut expected_recovery = if scrutinee_child.missing().is_some() {
        Some(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Scrutinee,
        })
    } else if arenas
        .expressions
        .resolve_prepared(slots, expression.scrutinee())
        .is_ok_and(HirExpr::is_poisoned)
    {
        Some(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Scrutinee,
            },
        ))
    } else {
        None
    };

    let Ok(parent_scope) = arenas.scopes.resolve_prepared(slots, payload.scope()) else {
        return false;
    };
    let mut arm_scopes = BTreeSet::new();
    for (arm_index, ((source_arm, attached_arm), arm)) in projection
        .arms()
        .iter()
        .zip(attached.match_arms())
        .zip(expression.arms())
        .enumerate()
    {
        let Ok(arm_index) = u32::try_from(arm_index) else {
            return false;
        };
        if attached_arm.projection() != source_arm {
            return false;
        }
        if !arm_scopes.insert(arm.scope())
            || !source_owner_matches(
                slots,
                arm.scope(),
                attached_arm.id(),
                &HirSourceSite::Span(attached_arm.whole_source_span()),
            )
        {
            return false;
        }
        let Ok(scope) = arenas.scopes.resolve_prepared(slots, arm.scope()) else {
            return false;
        };
        if scope.kind() != HirScopeKind::MatchArm
            || scope.parent() != Some(payload.scope())
            || scope.owner() != &HirScopeOwner::Expr(owner)
            || !parent_scope.children().contains(&arm.scope())
            || !source_owner_matches(
                slots,
                arm.pattern(),
                attached_arm.pattern().id(),
                &HirSourceSite::Span(attached_arm.pattern().whole_source_span()),
            )
        {
            return false;
        }
        let Ok(pattern) = arenas.patterns.resolve_prepared(slots, arm.pattern()) else {
            return false;
        };
        let Some(expected_locals) =
            canonical_pattern_locals(slots, arenas, arm.pattern(), arm.pattern(), arm.scope())
        else {
            return false;
        };
        let expected_local_ids = expected_locals
            .iter()
            .map(|expected| expected.local)
            .collect::<Vec<_>>();
        let mut generations = BTreeMap::new();
        let mut local_validation = BindingLocalValidation::new(
            arm.scope(),
            HirPatternBindingPolicy::MatchBinding,
            &mut generations,
            slots,
            arenas.patterns,
            arenas.locals,
        );
        if pattern.scope() != arm.scope()
            || expected_local_ids != arm.locals()
            || scope.locals() != expected_local_ids
            || expected_local_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != expected_local_ids.len()
            || !binding_locals_match(
                attached_arm.pattern(),
                &expected_locals,
                &mut local_validation,
            )
            || !expected_locals.iter().all(|expected| {
                arenas
                    .locals
                    .resolve_prepared(slots, expected.local)
                    .is_ok_and(|local| {
                        local.scope() == arm.scope()
                            && local.kind() == HirLocalKind::MatchBinding
                            && local.pattern() == Some(expected.pattern)
                    })
            })
        {
            return false;
        }
        if pattern.is_poisoned() {
            expected_recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild {
                    role: HirExprSourceRole::MatchArm {
                        arm: arm_index,
                        part: HirMatchArmSourcePart::Pattern,
                    },
                },
            ));
        }

        let guard_role = HirExprSourceRole::MatchArm {
            arm: arm_index,
            part: HirMatchArmSourcePart::Guard,
        };
        match (source_arm.guard(), attached_arm.guard(), arm.guard()) {
            (None, None, None) => {}
            (Some(SyntaxExpressionSlot::Authored), Some(attached_guard), Some(guard)) => {
                let Some(semantic) = attached_guard.authored_semantic().ok().flatten() else {
                    return false;
                };
                if !source_expression_matches(
                    slots,
                    arenas.expressions,
                    guard,
                    &semantic,
                    arm.scope(),
                ) {
                    return false;
                }
                if arenas
                    .expressions
                    .resolve_prepared(slots, guard)
                    .is_ok_and(HirExpr::is_poisoned)
                {
                    expected_recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild { role: guard_role },
                    ));
                }
            }
            (Some(SyntaxExpressionSlot::Missing), Some(attached_guard), None)
                if attached_guard.authored_semantic().ok().flatten().is_none()
                    && attached_guard.missing().is_some() =>
            {
                expected_recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild { role: guard_role },
                ));
            }
            _ => return false,
        }

        if !matches!(source_arm.arrow(), SyntaxRequiredTokenState::Present) {
            expected_recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild {
                    role: HirExprSourceRole::MatchArm {
                        arm: arm_index,
                        part: HirMatchArmSourcePart::Arrow,
                    },
                },
            ));
        }

        let value_role = HirExprSourceRole::MatchArm {
            arm: arm_index,
            part: HirMatchArmSourcePart::Value,
        };
        match source_arm.value() {
            SyntaxExpressionSlot::Authored => {
                let Some(semantic) = attached_arm.value().authored_semantic().ok().flatten() else {
                    return false;
                };
                if !source_expression_matches(
                    slots,
                    arenas.expressions,
                    arm.value(),
                    &semantic,
                    arm.scope(),
                ) {
                    return false;
                }
                if arenas
                    .expressions
                    .resolve_prepared(slots, arm.value())
                    .is_ok_and(HirExpr::is_poisoned)
                {
                    expected_recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild { role: value_role },
                    ));
                }
            }
            SyntaxExpressionSlot::Missing => {
                if attached_arm
                    .value()
                    .authored_semantic()
                    .ok()
                    .flatten()
                    .is_some()
                    || attached_arm.value().missing().is_none()
                    || !missing_scope_tail_matches(
                        parsed,
                        slots,
                        arenas.expressions,
                        arm.scope(),
                        arm.value(),
                        attached_arm.value().source_span(),
                    )
                {
                    return false;
                }
                expected_recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
            }
        }
    }

    match projection.terminator() {
        SyntaxMatchBodyTerminator::Closed => {}
        SyntaxMatchBodyTerminator::MissingBody => {
            expected_recovery.get_or_insert(HirRecoveryIssue::InvalidMatch(
                HirMatchRecoveryIssue::MissingBody,
            ));
        }
        SyntaxMatchBodyTerminator::RecoveredMissingClose => {
            expected_recovery.get_or_insert(HirRecoveryIssue::InvalidMatch(
                HirMatchRecoveryIssue::UnclosedBody,
            ));
        }
    }

    poison_state_matches(payload.state(), expected_recovery)
}
