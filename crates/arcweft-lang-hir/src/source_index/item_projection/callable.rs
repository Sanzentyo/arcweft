//! Shared exact-source validation for final callable item records.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedCallableContractClause, AttachedCallableParameter, AttachedCallableParameterKind,
    AttachedFixedParameterGroup, SyntaxNodeId,
};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use crate::identity::{
    ExprId, ItemId, LocalGeneration, LocalId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole,
    TypeId,
};
use crate::item::{HirFunctionParameterGroup, HirParameter, HirParameterKind};
use crate::scope::{HirLocalKind, HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::HirSourceSite;
use crate::source_index::block_projection::{
    BlockValidationArenas, source_expression_matches, source_owner_matches,
};
use crate::source_index::control_projection::canonical_pattern_locals;
use crate::source_index::pattern_projection::{BindingLocalValidation, binding_locals_match};

use super::{ItemValidationArenas, slot_is_poisoned, type_owner_matches};

#[derive(Clone, Copy)]
pub(super) struct CallableScopeSource<'a> {
    pub syntax: SyntaxNodeId,
    pub item: &'a SourceSpan,
    pub requires: &'a SourceSpan,
    pub ensures: &'a SourceSpan,
}

#[derive(Clone, Copy)]
pub(super) struct CallableScopeIds {
    pub item: ScopeId,
    pub callable: ScopeId,
    pub requires: ScopeId,
    pub ensures: ScopeId,
}

pub(super) fn contract_scopes_match(
    owner: ItemId,
    source: CallableScopeSource<'_>,
    ids: CallableScopeIds,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let Ok(callable) = arenas.scopes.resolve_prepared(slots, ids.callable) else {
        return false;
    };
    let Ok(requires) = arenas.scopes.resolve_prepared(slots, ids.requires) else {
        return false;
    };
    let Ok(ensures) = arenas.scopes.resolve_prepared(slots, ids.ensures) else {
        return false;
    };
    let Ok(requires_key) = SyntheticKey::try_new(
        SyntheticOwner::Item(owner),
        SyntheticRole::ContractRequiresScope,
        0,
    ) else {
        return false;
    };
    let Ok(ensures_key) = SyntheticKey::try_new(
        SyntheticOwner::Item(owner),
        SyntheticRole::ContractEnsuresScope,
        0,
    ) else {
        return false;
    };
    let Ok(requires_site) = HirSourceSite::from_attached_span(parsed.document(), source.requires)
    else {
        return false;
    };
    let Ok(ensures_site) = HirSourceSite::from_attached_span(parsed.document(), source.ensures)
    else {
        return false;
    };
    let synthetic_scope_matches = |scope, key: &SyntheticKey, site: &HirSourceSite| {
        slots.resolve_prepared(scope).is_ok_and(|metadata| {
            matches!(metadata.origin(), HirOrigin::Synthetic(actual) if actual == key)
                && metadata.source_site() == site
        })
    };

    source_owner_matches(
        slots,
        ids.callable,
        source.syntax,
        &HirSourceSite::Span(source.item.clone()),
    ) && callable.kind() == HirScopeKind::Callable
        && callable.parent() == Some(ids.item)
        && callable.owner() == &HirScopeOwner::Item(owner)
        && callable
            .children()
            .starts_with(&[ids.requires, ids.ensures])
        && arenas
            .scopes
            .resolve_prepared(slots, ids.item)
            .is_ok_and(|scope| scope.children().contains(&ids.callable))
        && synthetic_scope_matches(ids.requires, &requires_key, &requires_site)
        && synthetic_scope_matches(ids.ensures, &ensures_key, &ensures_site)
        && requires.kind() == HirScopeKind::ContractRequires
        && requires.parent() == Some(ids.callable)
        && requires.owner() == &HirScopeOwner::Item(owner)
        && requires.locals().is_empty()
        && scope_locals_are_exact(ids.requires, &[], slots, arenas)
        && ensures.kind() == HirScopeKind::ContractEnsures
        && ensures.parent() == Some(ids.callable)
        && ensures.owner() == &HirScopeOwner::Item(owner)
}

pub(super) fn item_callable_scope_matches(
    owner: ItemId,
    item_scope: ScopeId,
    callable_scope: ScopeId,
    syntax: SyntaxNodeId,
    source: &SourceSpan,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    source_owner_matches(
        slots,
        callable_scope,
        syntax,
        &HirSourceSite::Span(source.clone()),
    ) && arenas
        .scopes
        .resolve_prepared(slots, callable_scope)
        .is_ok_and(|scope| {
            scope.kind() == HirScopeKind::Callable
                && scope.parent() == Some(item_scope)
                && scope.owner() == &HirScopeOwner::Item(owner)
        })
        && arenas
            .scopes
            .resolve_prepared(slots, item_scope)
            .is_ok_and(|scope| scope.children().contains(&callable_scope))
}

/// Confirms that one item owns exactly the source-ordered callable scopes
/// projected by its inline members.
///
/// Direct child scopes may also belong to expressions in an item or member
/// prefix, so the source-ordered row filters those children by owner. The
/// arena-wide comparison is intentionally broader: after excluding the item
/// scope itself, these inline callable families admit no other scope owned by
/// the enclosing item anywhere in its subtree. This prevents an unreferenced
/// second body/callable scope from hiding below a valid method scope.
pub(super) fn item_owned_callable_scopes_are_exact(
    owner: ItemId,
    item_scope: ScopeId,
    expected: &[ScopeId],
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    if expected_set.len() != expected.len() {
        return false;
    }
    let Ok(item_scope_payload) = arenas.scopes.resolve_prepared(slots, item_scope) else {
        return false;
    };
    let source_ordered = item_scope_payload
        .children()
        .iter()
        .copied()
        .filter(|scope| {
            arenas
                .scopes
                .resolve_prepared(slots, *scope)
                .is_ok_and(|payload| {
                    payload.parent() == Some(item_scope)
                        && payload.owner() == &HirScopeOwner::Item(owner)
                })
        })
        .collect::<Vec<_>>();
    let Ok(all_scopes) = arenas.scopes.try_iter_prepared(slots) else {
        return false;
    };
    let item_owned = all_scopes
        .filter_map(|(scope, payload)| {
            (scope != item_scope && payload.owner() == &HirScopeOwner::Item(owner)).then_some(scope)
        })
        .collect::<BTreeSet<_>>();
    source_ordered == expected && item_owned == expected_set
}

/// Confirms that a scope's stored children are the complete backlink set in
/// strict source order. This spans child-owner kinds so an item or callable
/// cannot reorder method scopes around expression-owned scopes while retaining
/// individually valid children.
pub(super) fn scope_children_are_exact_in_source_order(
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let Ok(payload) = arenas.scopes.resolve_prepared(slots, scope) else {
        return false;
    };
    let expected = payload.children().iter().copied().collect::<BTreeSet<_>>();
    if expected.len() != payload.children().len() {
        return false;
    }
    let mut previous = None;
    for child in payload.children() {
        let Ok(metadata) = slots.resolve_prepared(*child) else {
            return false;
        };
        let current = source_order_key(metadata.source_site());
        if previous.is_some_and(|previous| previous >= current) {
            return false;
        }
        previous = Some(current);
    }
    let Ok(scopes) = arenas.scopes.try_iter_prepared(slots) else {
        return false;
    };
    let actual = scopes
        .filter_map(|(candidate, payload)| (payload.parent() == Some(scope)).then_some(candidate))
        .collect::<BTreeSet<_>>();
    expected == actual
}

fn source_order_key(site: &HirSourceSite) -> (usize, usize) {
    match site {
        HirSourceSite::Span(span) => (span.range().start(), span.range().end()),
        HirSourceSite::Insertion(insertion) => (insertion.offset(), insertion.offset()),
    }
}

pub(super) struct ParameterState {
    pub missing_type: bool,
    pub recovered: bool,
}

#[derive(Clone, Copy)]
pub(super) enum ParameterSurfacePolicy {
    Function,
    FixedOnly,
    FixedWithDefaults,
}

pub(super) fn function_parameter_groups_match(
    attached: &[AttachedFixedParameterGroup],
    retained: &[HirFunctionParameterGroup],
    callable_scope: ScopeId,
    policy: HirPatternBindingPolicy,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
    block_arenas: &BlockValidationArenas<'_>,
) -> Option<ParameterState> {
    if attached.len() != retained.len() {
        return None;
    }
    for (group_position, (attached, retained)) in attached.iter().zip(retained).enumerate() {
        if usize::from(attached.source_ordinal()) != group_position
            || attached.parameters().len() != retained.parameters().len()
            || attached
                .parameters()
                .iter()
                .enumerate()
                .any(|(parameter_position, parameter)| {
                    usize::from(parameter.group_ordinal()) != group_position
                        || usize::from(parameter.parameter_ordinal()) != parameter_position
                })
        {
            return None;
        }
    }
    parameters_match(
        attached
            .iter()
            .flat_map(AttachedFixedParameterGroup::parameters),
        attached
            .iter()
            .any(AttachedFixedParameterGroup::has_recovery),
        ParameterSurfacePolicy::Function,
        retained
            .iter()
            .flat_map(HirFunctionParameterGroup::parameters),
        callable_scope,
        policy,
        slots,
        arenas,
        block_arenas,
    )
}

pub(super) fn parameters_match<'a, 'b>(
    attached: impl IntoIterator<Item = &'a AttachedCallableParameter>,
    group_has_recovery: bool,
    surface: ParameterSurfacePolicy,
    retained: impl IntoIterator<Item = &'b HirParameter>,
    callable_scope: ScopeId,
    policy: HirPatternBindingPolicy,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
    block_arenas: &BlockValidationArenas<'_>,
) -> Option<ParameterState> {
    let attached = attached.into_iter().collect::<Vec<_>>();
    let retained = retained.into_iter().collect::<Vec<_>>();
    if attached.len() != retained.len() {
        return None;
    }
    let mut expected_locals = Vec::new();
    let mut generations = BTreeMap::new();
    let mut missing_type = false;
    let mut recovered = group_has_recovery;
    for (position, (attached, retained)) in attached.into_iter().zip(retained).enumerate() {
        let surface_recovery = match surface {
            ParameterSurfacePolicy::Function => {
                let kind_matches = matches!(
                    (attached.kind(), retained.kind()),
                    (
                        AttachedCallableParameterKind::Fixed,
                        HirParameterKind::Fixed
                    ) | (
                        AttachedCallableParameterKind::Rest { .. },
                        HirParameterKind::RestPositional
                    )
                );
                let default_matches = match (attached.default(), retained.default()) {
                    (None, None) => true,
                    (Some(attached), Some(retained)) => source_expression_matches(
                        slots,
                        block_arenas.expressions,
                        retained,
                        attached.value(),
                        callable_scope,
                    ),
                    _ => false,
                };
                if !kind_matches || !default_matches {
                    return None;
                }
                retained
                    .default()
                    .is_some_and(|default| slot_is_poisoned(slots, default))
            }
            ParameterSurfacePolicy::FixedOnly => {
                if retained.kind() != HirParameterKind::Fixed || retained.default().is_some() {
                    return None;
                }
                attached.is_rest() || attached.default().is_some()
            }
            ParameterSurfacePolicy::FixedWithDefaults => {
                if retained.kind() != HirParameterKind::Fixed {
                    return None;
                }
                let default_matches = match (attached.default(), retained.default()) {
                    (None, None) => true,
                    (Some(attached), Some(retained)) => source_expression_matches(
                        slots,
                        block_arenas.expressions,
                        retained,
                        attached.value(),
                        callable_scope,
                    ),
                    _ => false,
                };
                if !default_matches {
                    return None;
                }
                attached.is_rest()
                    || retained
                        .default()
                        .is_some_and(|default| slot_is_poisoned(slots, default))
            }
        };
        if usize::from(attached.source_ordinal()) != position
            || !source_owner_matches(
                slots,
                retained.pattern(),
                attached.pattern().id(),
                &HirSourceSite::Span(attached.pattern().whole_source_span()),
            )
            || !type_owner_matches(retained.ty(), attached.ty(), slots)
        {
            return None;
        }
        let pattern = arenas
            .patterns
            .resolve_prepared(slots, retained.pattern())
            .ok()?;
        let ty = arenas.types.resolve_prepared(slots, retained.ty()).ok()?;
        if pattern.scope() != callable_scope || ty.scope() != callable_scope {
            return None;
        }
        let locals = canonical_pattern_locals(
            slots,
            block_arenas,
            retained.pattern(),
            retained.pattern(),
            callable_scope,
        )?;
        let local_ids = locals.iter().map(|local| local.local).collect::<Vec<_>>();
        let mut local_validation = BindingLocalValidation::new(
            callable_scope,
            policy,
            &mut generations,
            slots,
            arenas.patterns,
            arenas.locals,
        );
        if retained.locals() != local_ids
            || !binding_locals_match(attached.pattern(), &locals, &mut local_validation)
        {
            return None;
        }
        expected_locals.extend(local_ids);
        missing_type |=
            ty.is_poisoned() && attached.ty().syntax().kind() == SyntaxKind::MissingType;
        recovered |= local_validation.is_poisoned()
            || pattern.is_poisoned()
            || ty.is_poisoned()
            || attached.has_recovery()
            || surface_recovery;
    }
    let callable = arenas.scopes.resolve_prepared(slots, callable_scope).ok()?;
    if callable.locals() != expected_locals
        || !scope_locals_are_exact(callable_scope, &expected_locals, slots, arenas)
    {
        return None;
    }
    Some(ParameterState {
        missing_type,
        recovered,
    })
}

pub(super) fn contracts_match(
    attached: &[AttachedCallableContractClause],
    retained_requires: &[ExprId],
    retained_ensures: &[ExprId],
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    let mut requires_position = 0_usize;
    let mut ensures_position = 0_usize;
    let mut recovered = false;
    for (source_position, clause) in attached.iter().enumerate() {
        if usize::from(clause.source_ordinal()) != source_position {
            return None;
        }
        let (retained, scope, family_position) = match clause {
            AttachedCallableContractClause::Requires { .. } => {
                let retained = *retained_requires.get(requires_position)?;
                let family_position = requires_position;
                requires_position += 1;
                (retained, requires_scope, family_position)
            }
            AttachedCallableContractClause::Ensures { .. } => {
                let retained = *retained_ensures.get(ensures_position)?;
                let family_position = ensures_position;
                ensures_position += 1;
                (retained, ensures_scope, family_position)
            }
        };
        if usize::from(clause.family_ordinal()) != family_position
            || !source_expression_matches(
                slots,
                arenas.expressions,
                retained,
                clause.condition(),
                scope,
            )
        {
            return None;
        }
        recovered |= clause.has_recovery() || slot_is_poisoned(slots, retained);
    }
    (requires_position == retained_requires.len() && ensures_position == retained_ensures.len())
        .then_some(recovered)
}

pub(super) fn postcondition_result_matches(
    expected_source: Option<SourceSpan>,
    ensures_scope_id: ScopeId,
    return_type: Option<TypeId>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let Ok(ensures_scope) = arenas.scopes.resolve_prepared(slots, ensures_scope_id) else {
        return false;
    };
    match expected_source {
        None => {
            ensures_scope.locals().is_empty()
                && scope_locals_are_exact(ensures_scope_id, &[], slots, arenas)
        }
        Some(source) => {
            let [local] = ensures_scope.locals() else {
                return false;
            };
            let Ok(key) = SyntheticKey::try_new(
                SyntheticOwner::Scope(ensures_scope_id),
                SyntheticRole::PostconditionResult,
                0,
            ) else {
                return false;
            };
            let Ok(site) = HirSourceSite::from_attached_span(parsed.document(), &source) else {
                return false;
            };
            let Ok(metadata) = slots.resolve_prepared(*local) else {
                return false;
            };
            let Ok(payload) = arenas.locals.resolve_prepared(slots, *local) else {
                return false;
            };
            matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
                && metadata.source_site() == &site
                && payload.scope() == ensures_scope_id
                && payload.kind() == HirLocalKind::PostconditionResult
                && payload.name().as_str() == "result"
                && payload.generation() == LocalGeneration::FIRST
                && payload.pattern().is_none()
                && payload.annotation() == return_type
                && !payload.is_mutable_binding()
                && !payload.is_poisoned()
                && scope_locals_are_exact(ensures_scope_id, &[*local], slots, arenas)
        }
    }
}

pub(super) fn direct_contract_children_are_exact(
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
    callable: &HirScope,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    if callable.children() != [requires_scope, ensures_scope] {
        return false;
    }
    let expected = callable.children().iter().copied().collect::<BTreeSet<_>>();
    let Ok(scopes) = arenas.scopes.try_iter_prepared(slots) else {
        return false;
    };
    let actual = scopes
        .filter_map(|(scope, payload)| (payload.parent() == Some(callable_scope)).then_some(scope))
        .collect::<BTreeSet<_>>();
    expected == actual
}

pub(super) fn direct_children_are_exact(
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
    item_body_scope: ScopeId,
    callable: &HirScope,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    if callable.children() != [requires_scope, ensures_scope, item_body_scope] {
        return false;
    }
    let source_ordered_children = callable.children().iter().copied().collect::<BTreeSet<_>>();
    if source_ordered_children.len() != callable.children().len()
        || !callable.children().iter().copied().all(|child| {
            arenas
                .scopes
                .resolve_prepared(slots, child)
                .is_ok_and(|payload| payload.parent() == Some(callable_scope))
        })
    {
        return false;
    }
    let Ok(scopes) = arenas.scopes.try_iter_prepared(slots) else {
        return false;
    };
    let backlinked_children = scopes
        .filter_map(|(scope, payload)| (payload.parent() == Some(callable_scope)).then_some(scope))
        .collect::<BTreeSet<_>>();
    source_ordered_children == backlinked_children
}

pub(super) fn item_body_scope_matches(
    owner: ItemId,
    callable_scope: ScopeId,
    body_scope: ScopeId,
    expected_kind: HirScopeKind,
    body_syntax: SyntaxNodeId,
    body_source: SourceSpan,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    source_owner_matches(
        slots,
        body_scope,
        body_syntax,
        &HirSourceSite::Span(body_source),
    ) && arenas
        .scopes
        .resolve_prepared(slots, body_scope)
        .is_ok_and(|payload| {
            payload.kind() == expected_kind
                && payload.parent() == Some(callable_scope)
                && payload.owner() == &HirScopeOwner::Item(owner)
        })
}

pub(super) fn scope_locals_are_exact(
    scope: ScopeId,
    expected: &[LocalId],
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let expected_len = expected.len();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if expected.len() != expected_len {
        return false;
    }
    let Ok(locals) = arenas.locals.try_iter_prepared(slots) else {
        return false;
    };
    let actual = locals
        .filter_map(|(local, payload)| (payload.scope() == scope).then_some(local))
        .collect::<BTreeSet<_>>();
    actual == expected
}
