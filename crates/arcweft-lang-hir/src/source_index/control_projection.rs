//! Cross-arena freeze validation for binding control expressions.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{AttachedExpressionChild, AttachedExpressionNode};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxExpressionSlot,
};
use arcweft_lang_syntax::incremental::ParsedSource;

use super::block_projection::{BlockValidationArenas, source_owner_matches};
use super::expression_manifest::expression_component_role;
use super::expression_manifest::projection::{expression_child_matches, poison_state_matches};
use super::pattern_projection::{BindingLocalValidation, binding_locals_match};
use super::{HirExprSourceRole, HirSourceSite};
use crate::arena::ArenaSnapshot;
use crate::expr::{
    HirClosureExpr, HirExpr, HirExprKind, HirExpressionRecoveryIssue, HirGenericExprIssue,
    HirIfLetExpr, HirPoisonState, HirRecoveryIssue,
};
use crate::identity::{
    ExprId, LocalGeneration, LocalId, PatternId, ScopeId, SyntheticKey, SyntheticOwner,
    SyntheticRole, TypeId,
};
use crate::pattern::{
    HirPatternBinding, HirPatternField, HirPatternKind, HirPatternSequenceRest,
    HirVariantPatternPayload,
};
use crate::scope::{HirLocalKind, HirPatternBindingPolicy, HirScopeKind, HirScopeOwner};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::type_ref::HirType;

/// Re-derives an `IfLet`'s asymmetric scope graph and canonical binding
/// publication from its attached source owner. Generic composite validation
/// cannot represent the outer-scope scrutinee/else and binding-scope
/// guard/then split.
#[allow(
    clippy::too_many_lines,
    reason = "one projection proves the complete asymmetric IfLet scope, binding, branch, and recovery graph"
)]
pub(super) fn if_let_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ExprId,
    payload: &HirExpr,
    expression: &HirIfLetExpr,
    attached: &AttachedExpressionNode,
) -> bool {
    let ExpressionProjection::IfLet {
        scrutinee,
        guard,
        then_branch,
        else_branch,
    } = attached.projection()
    else {
        return false;
    };
    let Some(attached_pattern) = attached.pattern() else {
        return false;
    };

    let binding_scope = expression.scope();
    let Ok(scope) = arenas.scopes.resolve_prepared(slots, binding_scope) else {
        return false;
    };
    let Ok(parent_scope) = arenas.scopes.resolve_prepared(slots, payload.scope()) else {
        return false;
    };
    if scope.kind() != HirScopeKind::Conditional
        || scope.parent() != Some(payload.scope())
        || scope.owner() != &HirScopeOwner::Expr(owner)
        || !parent_scope.children().contains(&binding_scope)
        || !source_owner_matches(
            slots,
            binding_scope,
            attached.id(),
            &HirSourceSite::Span(attached.whole_source_span()),
        )
        || !source_owner_matches(
            slots,
            expression.pattern(),
            attached_pattern.id(),
            &HirSourceSite::Span(attached_pattern.whole_source_span()),
        )
    {
        return false;
    }

    let Some(expected_locals) = canonical_pattern_locals(
        slots,
        arenas,
        expression.pattern(),
        expression.pattern(),
        binding_scope,
    ) else {
        return false;
    };
    let local_ids = expected_locals
        .iter()
        .map(|expected| expected.local)
        .collect::<Vec<_>>();
    let unique_locals = local_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut generations = BTreeMap::new();
    let mut local_validation = BindingLocalValidation::new(
        binding_scope,
        HirPatternBindingPolicy::PatternBinding,
        &mut generations,
        slots,
        arenas.patterns,
        arenas.locals,
    );
    if unique_locals.len() != local_ids.len()
        || scope.locals() != local_ids
        || !binding_locals_match(attached_pattern, &expected_locals, &mut local_validation)
    {
        return false;
    }
    if !expected_locals.iter().all(|expected| {
        arenas
            .locals
            .resolve_prepared(slots, expected.local)
            .is_ok_and(|local| {
                local.scope() == binding_scope
                    && local.kind() == HirLocalKind::PatternBinding
                    && local.pattern() == Some(expected.pattern)
            })
    }) {
        return false;
    }

    let mut expected_ordinals = vec![0, 2];
    if guard.is_some() {
        expected_ordinals.push(1);
    }
    if else_branch.is_some() {
        expected_ordinals.push(3);
    }
    expected_ordinals.sort_unstable();
    let actual_ordinals = attached
        .children()
        .iter()
        .map(AttachedExpressionChild::ordinal)
        .collect::<Vec<_>>();
    let actual_unique = actual_ordinals.iter().copied().collect::<BTreeSet<_>>();
    if actual_unique.len() != actual_ordinals.len()
        || actual_unique != expected_ordinals.iter().copied().collect()
    {
        return false;
    }

    let mut expected_recovery = match arenas
        .patterns
        .resolve_prepared(slots, expression.pattern())
        .ok()
        .map(crate::pattern::HirPattern::state)
    {
        Some(HirPoisonState::Clean) => None,
        Some(HirPoisonState::Poisoned(issue)) => Some(issue.clone()),
        None => return false,
    };

    let Some(scrutinee_child) =
        child_for_slot(attached, ExpressionComponentRole::Scrutinee, *scrutinee)
    else {
        return false;
    };
    let Some(recovery) = child_recovery(
        parsed,
        slots,
        arenas,
        owner,
        payload.scope(),
        attached,
        scrutinee_child,
        expression.scrutinee(),
    ) else {
        return false;
    };
    expected_recovery = expected_recovery.or(recovery);

    if let Some(guard_slot) = guard {
        let Some(guard_child) =
            child_for_slot(attached, ExpressionComponentRole::Guard, *guard_slot)
        else {
            return false;
        };
        let Some(recovery) = expression.guard().and_then(|guard_owner| {
            child_recovery(
                parsed,
                slots,
                arenas,
                owner,
                binding_scope,
                attached,
                guard_child,
                guard_owner,
            )
        }) else {
            return false;
        };
        expected_recovery = expected_recovery.or(recovery);
    } else if expression.guard().is_some() {
        return false;
    }

    let Some(then_child) =
        child_for_slot(attached, ExpressionComponentRole::ThenBranch, *then_branch)
    else {
        return false;
    };
    let Some(recovery) = child_recovery(
        parsed,
        slots,
        arenas,
        owner,
        binding_scope,
        attached,
        then_child,
        expression.then_branch(),
    ) else {
        return false;
    };
    expected_recovery = expected_recovery.or(recovery);

    let else_recovery = if let Some(slot) = else_branch {
        let Some(child) = child_for_slot(attached, ExpressionComponentRole::ElseBranch, *slot)
        else {
            return false;
        };
        if child.missing().is_some() {
            if !missing_required_tail_matches(
                parsed,
                slots,
                arenas,
                owner,
                expression.else_branch(),
                payload.scope(),
                &child.source_span(),
            ) {
                return false;
            }
            Some(HirRecoveryIssue::MissingRequiredTail)
        } else {
            let Some(recovery) = child_recovery(
                parsed,
                slots,
                arenas,
                owner,
                payload.scope(),
                attached,
                child,
                expression.else_branch(),
            ) else {
                return false;
            };
            recovery
        }
    } else {
        let Some(source) = attached.component(ExpressionComponentRole::ElseBranch) else {
            return false;
        };
        if !missing_required_tail_matches(
            parsed,
            slots,
            arenas,
            owner,
            expression.else_branch(),
            payload.scope(),
            &source,
        ) {
            return false;
        }
        Some(HirRecoveryIssue::MissingRequiredTail)
    };
    expected_recovery = expected_recovery.or(else_recovery);
    poison_state_matches(payload.state(), expected_recovery)
}

/// Re-derives one Closure's parameter bindings, annotations, lexical scope,
/// and body from the accepted attached owner. The final expression and scope
/// arenas are the only semantic carriers; no callback-specific syntax record
/// participates in this validation.
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "one projection proves closure parameters, captures, body scope, source roles, and recovery together"
)]
pub(super) fn closure_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    types: &ArenaSnapshot<HirType, TypeId>,
    owner: ExprId,
    payload: &HirExpr,
    expression: &HirClosureExpr,
    attached: &AttachedExpressionNode,
) -> bool {
    let ExpressionProjection::Closure(projection) = attached.projection() else {
        return false;
    };
    if expression.parameters().len() != attached.closure_parameters().len()
        || projection.parameters().len() != expression.parameters().len()
        || expression.result_type().is_some() != attached.closure_result_type().is_some()
    {
        return false;
    }

    let closure_scope = expression.scope();
    let Ok(scope) = arenas.scopes.resolve_prepared(slots, closure_scope) else {
        return false;
    };
    let Ok(parent_scope) = arenas.scopes.resolve_prepared(slots, payload.scope()) else {
        return false;
    };
    if scope.kind() != HirScopeKind::Closure
        || scope.parent() != Some(payload.scope())
        || scope.owner() != &HirScopeOwner::Expr(owner)
        || !parent_scope.children().contains(&closure_scope)
        || !source_owner_matches(
            slots,
            closure_scope,
            attached.id(),
            &HirSourceSite::Span(attached.whole_source_span()),
        )
    {
        return false;
    }

    let mut expected_locals = Vec::new();
    let mut expected_recovery = None;
    let mut generations = BTreeMap::<crate::leaf::HirName, LocalGeneration>::new();
    for ((parameter, attached_parameter), syntax_parameter) in expression
        .parameters()
        .iter()
        .zip(attached.closure_parameters())
        .zip(projection.parameters())
    {
        if parameter.local_scope() != closure_scope
            || parameter.ty().is_some() != syntax_parameter.has_type()
            || parameter.ty().is_some() != attached_parameter.ty().is_some()
            || !source_owner_matches(
                slots,
                parameter.pattern(),
                attached_parameter.pattern().id(),
                &HirSourceSite::Span(attached_parameter.pattern().whole_source_span()),
            )
        {
            return false;
        }
        let Ok(pattern) = arenas.patterns.resolve_prepared(slots, parameter.pattern()) else {
            return false;
        };
        match pattern.state() {
            HirPoisonState::Clean => {}
            HirPoisonState::Poisoned(issue) => {
                expected_recovery.get_or_insert_with(|| issue.clone());
            }
        }
        let Some(parameter_locals) = canonical_pattern_locals(
            slots,
            arenas,
            parameter.pattern(),
            parameter.pattern(),
            closure_scope,
        ) else {
            return false;
        };
        let mut local_validation = BindingLocalValidation::new(
            closure_scope,
            HirPatternBindingPolicy::ClosureParameter,
            &mut generations,
            slots,
            arenas.patterns,
            arenas.locals,
        );
        if !binding_locals_match(
            attached_parameter.pattern(),
            &parameter_locals,
            &mut local_validation,
        ) || !parameter_locals.iter().all(|expected| {
            arenas
                .locals
                .resolve_prepared(slots, expected.local)
                .is_ok_and(|local| {
                    local.scope() == closure_scope
                        && local.kind() == HirLocalKind::ClosureParameter
                        && local.pattern() == Some(expected.pattern)
                })
        }) {
            return false;
        }
        expected_locals.extend(parameter_locals.iter().map(|expected| expected.local));

        match (parameter.ty(), attached_parameter.ty()) {
            (None, None) => {}
            (Some(ty), Some(attached_type)) => {
                if !source_owner_matches(
                    slots,
                    ty,
                    attached_type.id(),
                    &HirSourceSite::Span(attached_type.whole_source_span()),
                ) {
                    return false;
                }
                let Ok(payload) = types.resolve_prepared(slots, ty) else {
                    return false;
                };
                if payload.scope() != closure_scope {
                    return false;
                }
                if let HirPoisonState::Poisoned(issue) = payload.state() {
                    expected_recovery.get_or_insert_with(|| issue.clone());
                }
            }
            _ => return false,
        }
    }
    if expected_locals
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != expected_locals.len()
        || scope.locals() != expected_locals
    {
        return false;
    }

    match (expression.result_type(), attached.closure_result_type()) {
        (None, None) => {}
        (Some(ty), Some(attached_type)) => {
            if !source_owner_matches(
                slots,
                ty,
                attached_type.id(),
                &HirSourceSite::Span(attached_type.whole_source_span()),
            ) {
                return false;
            }
            let Ok(result) = types.resolve_prepared(slots, ty) else {
                return false;
            };
            if result.scope() != closure_scope {
                return false;
            }
            if let HirPoisonState::Poisoned(issue) = result.state() {
                expected_recovery.get_or_insert_with(|| issue.clone());
            }
        }
        _ => return false,
    }

    let Some(body) = child_for_slot(attached, ExpressionComponentRole::Body, projection.body())
    else {
        return false;
    };
    if attached.children().len() != 1 {
        return false;
    }
    let body_recovery = if body.missing().is_some() {
        if !missing_required_tail_matches(
            parsed,
            slots,
            arenas,
            owner,
            expression.body(),
            closure_scope,
            &body.source_span(),
        ) {
            return false;
        }
        Some(HirRecoveryIssue::MissingRequiredTail)
    } else {
        if !expression_child_matches(
            parsed,
            slots,
            arenas.expressions,
            owner,
            closure_scope,
            attached,
            body,
            expression.body(),
        ) {
            return false;
        }
        if arenas
            .expressions
            .resolve_prepared(slots, expression.body())
            .is_ok_and(HirExpr::is_poisoned)
        {
            Some(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild {
                    role: HirExprSourceRole::Body,
                },
            ))
        } else {
            None
        }
    };
    expected_recovery = expected_recovery.or(body_recovery);
    poison_state_matches(payload.state(), expected_recovery)
}

fn child_for_slot(
    attached: &AttachedExpressionNode,
    role: ExpressionComponentRole,
    slot: SyntaxExpressionSlot,
) -> Option<&AttachedExpressionChild> {
    attached
        .children()
        .iter()
        .find(|child| child.component_role() == role)
        .filter(|child| {
            child.authored().is_some() == matches!(slot, SyntaxExpressionSlot::Authored)
        })
}

#[allow(
    clippy::too_many_arguments,
    clippy::option_option,
    reason = "the helper returns a deliberate tri-state: mismatch, clean child, or a typed child recovery issue"
)]
fn child_recovery(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    parent: ExprId,
    scope: ScopeId,
    parent_attached: &AttachedExpressionNode,
    attached: &AttachedExpressionChild,
    child: ExprId,
) -> Option<Option<HirRecoveryIssue>> {
    let role = expression_component_role(parent_attached.projection(), attached.component_role())?;
    if !expression_child_matches(
        parsed,
        slots,
        arenas.expressions,
        parent,
        scope,
        parent_attached,
        attached,
        child,
    ) {
        return None;
    }
    if attached.missing().is_some() {
        return Some(Some(HirRecoveryIssue::MissingOperand { role }));
    }
    arenas
        .expressions
        .resolve_prepared(slots, child)
        .ok()
        .map(|payload| {
            payload
                .is_poisoned()
                .then_some(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild { role },
                ))
        })
}

fn missing_required_tail_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    parent: ExprId,
    owner: ExprId,
    scope: ScopeId,
    source: &arcweft_source::SourceSpan,
) -> bool {
    let Ok(key) = SyntheticKey::try_new(
        SyntheticOwner::Expr(parent),
        SyntheticRole::MissingRequiredTail,
        0,
    ) else {
        return false;
    };
    let Ok(expected_site) = HirSourceSite::from_attached_span(parsed.document(), source) else {
        return false;
    };
    let Ok(metadata) = slots.resolve_prepared(owner) else {
        return false;
    };
    let Ok(payload) = arenas.expressions.resolve_prepared(slots, owner) else {
        return false;
    };
    matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
        && metadata.source_site() == &expected_site
        && payload.scope() == scope
        && matches!(
            (payload.kind(), payload.state()),
            (
                HirExprKind::Error(error),
                HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
            ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure
        )
}

#[derive(Clone, Copy)]
pub(super) struct ExpectedLocal {
    pub(super) local: LocalId,
    pub(super) pattern: PatternId,
}

pub(super) fn canonical_pattern_locals(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    root: PatternId,
    owner: PatternId,
    scope: ScopeId,
) -> Option<Vec<ExpectedLocal>> {
    let pattern = arenas.patterns.resolve_prepared(slots, owner).ok()?;
    if pattern.scope() != scope {
        return None;
    }
    match pattern.kind() {
        HirPatternKind::Binding(binding)
        | HirPatternKind::MutableBinding(binding)
        | HirPatternKind::TypedBinding { binding, .. } => {
            Some(binding_local(binding, root).into_iter().collect())
        }
        HirPatternKind::Literal(_)
        | HirPatternKind::EntityReference(_)
        | HirPatternKind::Discard
        | HirPatternKind::Error(_) => Some(Vec::new()),
        HirPatternKind::Variant(variant) => match variant.payload() {
            HirVariantPatternPayload::Absent
            | HirVariantPatternPayload::Recovered { pattern: None, .. } => Some(Vec::new()),
            HirVariantPatternPayload::Pattern(child)
            | HirVariantPatternPayload::Recovered {
                pattern: Some(child),
                ..
            } => canonical_pattern_locals(slots, arenas, root, *child, scope),
        },
        HirPatternKind::Tuple { elements } => {
            child_pattern_locals(slots, arenas, root, elements, scope)
        }
        HirPatternKind::Record { fields, .. } => {
            let mut locals = Vec::new();
            for field in fields {
                match field {
                    HirPatternField::Explicit { pattern, .. } => locals.extend(
                        canonical_pattern_locals(slots, arenas, root, *pattern, scope)?,
                    ),
                    HirPatternField::Shorthand { local, .. } => locals.push(ExpectedLocal {
                        local: *local,
                        pattern: root,
                    }),
                    HirPatternField::Rest {
                        binding: Some(local),
                    } => locals.push(ExpectedLocal {
                        local: *local,
                        pattern: owner,
                    }),
                    HirPatternField::Rest { binding: None } | HirPatternField::Invalid { .. } => {}
                }
            }
            Some(locals)
        }
        HirPatternKind::BracketSequence { elements, rest } => {
            let mut locals = child_pattern_locals(slots, arenas, root, elements, scope)?;
            if let HirPatternSequenceRest::Bound(local) = rest {
                locals.push(ExpectedLocal {
                    local: *local,
                    pattern: owner,
                });
            }
            Some(locals)
        }
        HirPatternKind::WholeBinding { binding, pattern } => {
            let mut locals = binding_local(binding, root).into_iter().collect::<Vec<_>>();
            locals.extend(canonical_pattern_locals(
                slots, arenas, root, *pattern, scope,
            )?);
            Some(locals)
        }
        HirPatternKind::Or { alternatives } => {
            let (first, rest) = alternatives.split_first()?;
            let canonical = canonical_pattern_locals(slots, arenas, root, *first, scope)?;
            let canonical_ids = canonical
                .iter()
                .map(|local| local.local)
                .collect::<Vec<_>>();
            for alternative in rest {
                let actual = canonical_pattern_locals(slots, arenas, root, *alternative, scope)?;
                if actual.iter().map(|local| local.local).collect::<Vec<_>>() != canonical_ids {
                    return None;
                }
            }
            Some(canonical)
        }
    }
}

fn child_pattern_locals(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    root: PatternId,
    children: &[PatternId],
    scope: ScopeId,
) -> Option<Vec<ExpectedLocal>> {
    let mut locals = Vec::new();
    for child in children {
        locals.extend(canonical_pattern_locals(
            slots, arenas, root, *child, scope,
        )?);
    }
    Some(locals)
}

fn binding_local(binding: &HirPatternBinding, pattern: PatternId) -> Option<ExpectedLocal> {
    match binding {
        HirPatternBinding::Bound { local, .. } => Some(ExpectedLocal {
            local: *local,
            pattern,
        }),
        HirPatternBinding::Recovered { .. } => None,
    }
}
