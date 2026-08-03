//! Activity-specific re-derivation for final item publication.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedActivityContractClause, AttachedActivityContractEntry, AttachedActivityDeclaration,
    AttachedActivityEntry, AttachedActivityLifecycle, AttachedActivityMode, AttachedActivityPort,
    AttachedRequiredName,
};

use crate::identity::{
    ItemId, LocalGeneration, LocalId, SyntheticKey, SyntheticOwner, SyntheticRole,
};
use crate::item::{
    HirActivityDeclaration, HirActivityLifecycle, HirActivityMode, HirActivityPortMember,
    HirDeclarationMember, HirDeclarationMemberArena, HirDeclarationMemberId,
    HirDeclarationMemberIssue, HirDeclarationMemberKind, HirDeclarationMemberPoisonState, HirItem,
    HirItemFamily, HirItemIssue, HirItemKind, HirItemPoisonState, HirRequiredName,
};
use crate::leaf::HirName;
use crate::scope::{HirLocalKind, HirScopeKind, HirScopeOwner};
use crate::slot::HirOrigin;
use crate::source_index::HirSourceSite;

use super::{
    ItemValidationArenas, item_prefix_matches, item_state, prefix_issue, required_name_matches,
    retained_header_item_issue, retained_header_matches, slot_is_poisoned, source_matches,
    type_is_poisoned, type_owner_matches,
};

#[derive(Clone, Copy)]
enum PortDirection {
    Input,
    Output,
}

pub(super) fn payload_matches(
    owner: ItemId,
    attached: &AttachedActivityDeclaration,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Activity(activity) = item.kind() else {
        return false;
    };
    if !item_prefix_matches(item, attached.prefix(), slots)
        || !retained_header_matches(activity.header(), attached.header())
        || !contract_scopes_match(owner, attached, item, activity, slots, arenas)
    {
        return false;
    }

    let retained_members = match members {
        Some(members)
            if members.owner() == owner && members.family() == HirItemFamily::Activity =>
        {
            members.members()
        }
        Some(_) => return false,
        None => &[],
    };
    let mut member_position = 0_usize;
    let mut input_ids = Vec::new();
    let mut output_ids = Vec::new();
    let mut expected_locals = Vec::new();
    let mut generations = BTreeMap::<HirName, u32>::new();
    let mut requires_position = 0_usize;
    let mut ensures_position = 0_usize;
    let mut expected_mode = HirActivityMode::Deterministic;
    let mut expected_lifecycle = HirActivityLifecycle::Stateless;
    let mut mode_selected = false;
    let mut lifecycle_selected = false;
    let mut first_body_issue = None;

    for (entry_position, entry) in attached.body().entries().iter().enumerate() {
        if usize::from(entry.source_ordinal()) != entry_position {
            return false;
        }
        let entry_has_issue = match entry {
            AttachedActivityEntry::Mode(member) => {
                if !mode_selected {
                    expected_mode = mode(member.value());
                    mode_selected = true;
                }
                member.state().has_recovery()
                    || member.assignment().is_missing()
                    || member.value().has_recovery()
            }
            AttachedActivityEntry::Lifecycle(member) => {
                if !lifecycle_selected {
                    expected_lifecycle = lifecycle(member.value());
                    lifecycle_selected = true;
                }
                member.state().has_recovery()
                    || member.assignment().is_missing()
                    || member.value().has_recovery()
            }
            AttachedActivityEntry::Input(section) => {
                let mut issue = section.state().has_recovery() || section.body().has_recovery();
                if !ports_match(
                    owner,
                    section.body().ports(),
                    PortDirection::Input,
                    retained_members,
                    &mut member_position,
                    &mut input_ids,
                    &mut expected_locals,
                    &mut generations,
                    activity.scopes().callable(),
                    slots,
                    arenas,
                    &mut issue,
                ) {
                    return false;
                }
                issue
            }
            AttachedActivityEntry::Output(section) => {
                let mut issue = section.state().has_recovery() || section.body().has_recovery();
                if !ports_match(
                    owner,
                    section.body().ports(),
                    PortDirection::Output,
                    retained_members,
                    &mut member_position,
                    &mut output_ids,
                    &mut expected_locals,
                    &mut generations,
                    activity.scopes().callable(),
                    slots,
                    arenas,
                    &mut issue,
                ) {
                    return false;
                }
                issue
            }
            AttachedActivityEntry::Contract(section) => {
                let mut issue = section.state().has_recovery() || section.body().has_recovery();
                for (clause_position, clause) in section.body().entries().iter().enumerate() {
                    if usize::from(clause.source_ordinal()) != clause_position {
                        return false;
                    }
                    match clause {
                        AttachedActivityContractEntry::Clause(clause) => {
                            let (retained, scope) = match clause.as_ref() {
                                AttachedActivityContractClause::Requires { .. } => {
                                    let Some(retained) =
                                        activity.requires().get(requires_position).copied()
                                    else {
                                        return false;
                                    };
                                    requires_position += 1;
                                    (retained, activity.scopes().requires())
                                }
                                AttachedActivityContractClause::Ensures { .. } => {
                                    let Some(retained) =
                                        activity.ensures().get(ensures_position).copied()
                                    else {
                                        return false;
                                    };
                                    ensures_position += 1;
                                    (retained, activity.scopes().ensures())
                                }
                            };
                            let expression = clause.condition().expression();
                            let Ok(payload) = arenas.expressions.resolve_prepared(slots, retained)
                            else {
                                return false;
                            };
                            if !source_matches(slots, retained, expression.id())
                                || payload.scope() != scope
                            {
                                return false;
                            }
                            issue |= clause.is_out_of_order() || slot_is_poisoned(slots, retained);
                        }
                        AttachedActivityContractEntry::Recovery { .. } => issue = true,
                    }
                }
                issue
            }
            AttachedActivityEntry::Recovery { .. } => true,
        };
        if entry_has_issue {
            first_body_issue.get_or_insert(HirItemIssue::InvalidMember);
        }
    }

    let member_ids_match = member_position == retained_members.len()
        && member_position == item.members().len()
        && item
            .members()
            .iter()
            .copied()
            .enumerate()
            .all(|(position, id)| {
                u32::try_from(position)
                    .is_ok_and(|ordinal| id == HirDeclarationMemberId::new(owner, ordinal))
            });
    let member_arena_presence_matches = (member_position == 0) == members.is_none();
    let contract_counts_match = requires_position == activity.requires().len()
        && ensures_position == activity.ensures().len();
    let scope_locals_match = arenas
        .scopes
        .resolve_prepared(slots, activity.scopes().callable())
        .is_ok_and(|scope| scope.locals() == expected_locals.as_slice())
        && exact_callable_locals_match(
            activity.scopes().callable(),
            &expected_locals,
            slots,
            arenas,
        );
    let expected_state = activity_state(attached, item, first_body_issue, slots);

    member_ids_match
        && member_arena_presence_matches
        && contract_counts_match
        && scope_locals_match
        && activity.inputs() == input_ids.as_slice()
        && activity.outputs() == output_ids.as_slice()
        && activity.mode() == expected_mode
        && activity.lifecycle() == expected_lifecycle
        && item.state() == &expected_state
}

#[allow(clippy::too_many_arguments)]
fn ports_match(
    owner: ItemId,
    attached: &[AttachedActivityPort],
    direction: PortDirection,
    retained: &[HirDeclarationMember],
    member_position: &mut usize,
    direction_ids: &mut Vec<HirDeclarationMemberId>,
    expected_locals: &mut Vec<LocalId>,
    generations: &mut BTreeMap<HirName, u32>,
    callable_scope: crate::identity::ScopeId,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
    entry_has_issue: &mut bool,
) -> bool {
    for (port_position, attached) in attached.iter().enumerate() {
        if usize::from(attached.source_ordinal()) != port_position {
            return false;
        }
        let Ok(ordinal) = u32::try_from(*member_position) else {
            return false;
        };
        let expected_id = HirDeclarationMemberId::new(owner, ordinal);
        let Some(retained) = retained.get(*member_position) else {
            return false;
        };
        let retained_port = match (direction, retained.kind()) {
            (PortDirection::Input, HirDeclarationMemberKind::ActivityInput(port))
            | (PortDirection::Output, HirDeclarationMemberKind::ActivityOutput(port)) => port,
            _ => return false,
        };
        if retained.id() != expected_id
            || !activity_port_matches(
                attached,
                retained_port,
                retained.state(),
                callable_scope,
                expected_locals,
                generations,
                slots,
                arenas,
            )
        {
            return false;
        }
        *entry_has_issue |= retained.is_poisoned();
        direction_ids.push(expected_id);
        *member_position += 1;
    }
    true
}

fn activity_port_matches(
    attached: &AttachedActivityPort,
    retained: &HirActivityPortMember,
    state: HirDeclarationMemberPoisonState,
    callable_scope: crate::identity::ScopeId,
    expected_locals: &mut Vec<LocalId>,
    generations: &mut BTreeMap<HirName, u32>,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    if !required_name_matches(retained.name(), attached.name())
        || !type_owner_matches(retained.ty(), attached.ty(), slots)
    {
        return false;
    }
    let expected_state = port_state(attached, retained.ty(), slots);
    if state != expected_state {
        return false;
    }
    match (retained.name(), retained.local(), attached.name()) {
        (
            HirRequiredName::Resolved(name),
            Some(local),
            AttachedRequiredName::Resolved { syntax, .. },
        ) => {
            let generation = generations.entry(name.clone()).or_default();
            let Some(next_generation) = generation.checked_add(1) else {
                return false;
            };
            *generation = next_generation;
            let Some(expected_generation) = LocalGeneration::try_new(next_generation) else {
                return false;
            };
            let Ok(payload) = arenas.locals.resolve_prepared(slots, local) else {
                return false;
            };
            let site_matches = slots.resolve_prepared(local).is_ok_and(|metadata| {
                metadata.source_site() == &HirSourceSite::Span(syntax.source_span())
            });
            if !source_matches(slots, local, syntax.id())
                || !site_matches
                || payload.scope() != callable_scope
                || payload.kind() != HirLocalKind::Parameter
                || payload.name() != name
                || payload.generation() != expected_generation
                || payload.pattern().is_some()
                || payload.annotation() != Some(retained.ty())
                || payload.is_mutable_binding()
                || payload.is_poisoned() != state.is_poisoned()
            {
                return false;
            }
            expected_locals.push(local);
            true
        }
        (HirRequiredName::Missing, None, AttachedRequiredName::Missing { .. }) => true,
        _ => false,
    }
}

fn exact_callable_locals_match(
    callable_scope: crate::identity::ScopeId,
    expected: &[LocalId],
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    if expected_set.len() != expected.len() {
        return false;
    }
    let Ok(locals) = arenas.locals.try_iter_prepared(slots) else {
        return false;
    };
    let actual = locals
        .filter_map(|(local, payload)| (payload.scope() == callable_scope).then_some(local))
        .collect::<BTreeSet<_>>();
    actual == expected_set
}

fn contract_scopes_match(
    owner: ItemId,
    attached: &AttachedActivityDeclaration,
    item: &HirItem,
    activity: &HirActivityDeclaration,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let scopes = activity.scopes();
    let Ok(callable) = arenas.scopes.resolve_prepared(slots, scopes.callable()) else {
        return false;
    };
    let Ok(requires) = arenas.scopes.resolve_prepared(slots, scopes.requires()) else {
        return false;
    };
    let Ok(ensures) = arenas.scopes.resolve_prepared(slots, scopes.ensures()) else {
        return false;
    };
    let Some(requires_key) = SyntheticKey::try_new(
        SyntheticOwner::Item(owner),
        SyntheticRole::ContractRequiresScope,
        0,
    )
    .ok() else {
        return false;
    };
    let Some(ensures_key) = SyntheticKey::try_new(
        SyntheticOwner::Item(owner),
        SyntheticRole::ContractEnsuresScope,
        0,
    )
    .ok() else {
        return false;
    };
    let item_site = HirSourceSite::Span(attached.syntax().source_span());
    let requires_site = HirSourceSite::Span(attached.requires_scope_source_span());
    let ensures_site = HirSourceSite::Span(attached.ensures_scope_source_span());
    let synthetic_matches = |scope, expected, expected_site: &HirSourceSite| {
        slots.resolve_prepared(scope).is_ok_and(|metadata| {
            matches!(metadata.origin(), HirOrigin::Synthetic(actual) if actual == expected)
                && metadata.source_site() == expected_site
        })
    };
    source_matches(slots, scopes.callable(), attached.syntax().id())
        && slots
            .resolve_prepared(scopes.callable())
            .is_ok_and(|metadata| metadata.source_site() == &item_site)
        && callable.kind() == HirScopeKind::Callable
        && callable.parent() == Some(item.scope())
        && callable.owner() == &HirScopeOwner::Item(owner)
        && callable.children() == [scopes.requires(), scopes.ensures()]
        && arenas
            .scopes
            .resolve_prepared(slots, item.scope())
            .is_ok_and(|root| root.children().contains(&scopes.callable()))
        && synthetic_matches(scopes.requires(), &requires_key, &requires_site)
        && synthetic_matches(scopes.ensures(), &ensures_key, &ensures_site)
        && requires.kind() == HirScopeKind::ContractRequires
        && requires.parent() == Some(scopes.callable())
        && requires.owner() == &HirScopeOwner::Item(owner)
        && requires.locals().is_empty()
        && ensures.kind() == HirScopeKind::ContractEnsures
        && ensures.parent() == Some(scopes.callable())
        && ensures.owner() == &HirScopeOwner::Item(owner)
        && ensures.locals().is_empty()
}

fn activity_state(
    attached: &AttachedActivityDeclaration,
    item: &HirItem,
    first_body_issue: Option<HirItemIssue>,
    slots: &crate::slot::SlotSnapshot,
) -> HirItemPoisonState {
    item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| retained_header_item_issue(attached.header()))
            .or_else(|| {
                attached
                    .unexpected_header_recovery()
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .body()
                    .is_missing()
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(first_body_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                attached
                    .trailing_recovery()
                    .is_some()
                    .then_some(HirItemIssue::Recovery)
            }),
    )
}

fn port_state(
    attached: &AttachedActivityPort,
    ty: crate::identity::TypeId,
    slots: &crate::slot::SlotSnapshot,
) -> HirDeclarationMemberPoisonState {
    let issue = if attached.is_duplicate() {
        Some(HirDeclarationMemberIssue::Duplicate)
    } else if attached.name().is_missing()
        || attached.colon().is_missing()
        || type_is_poisoned(ty, slots)
        || attached.initializer_recovery().is_some()
    {
        Some(HirDeclarationMemberIssue::RecoveredChild)
    } else {
        None
    };
    issue.map_or(
        HirDeclarationMemberPoisonState::Clean,
        HirDeclarationMemberPoisonState::Poisoned,
    )
}

const fn mode(value: &AttachedActivityMode) -> HirActivityMode {
    match value {
        AttachedActivityMode::Deterministic(_) => HirActivityMode::Deterministic,
        AttachedActivityMode::CheckpointedRealtime(_) => HirActivityMode::CheckpointedRealtime,
        AttachedActivityMode::ExternalRealtime(_) => HirActivityMode::ExternalRealtime,
        AttachedActivityMode::Missing(_) | AttachedActivityMode::Invalid(_) => {
            HirActivityMode::Deterministic
        }
    }
}

const fn lifecycle(value: &AttachedActivityLifecycle) -> HirActivityLifecycle {
    match value {
        AttachedActivityLifecycle::Stateless(_) => HirActivityLifecycle::Stateless,
        AttachedActivityLifecycle::Snapshot(_) => HirActivityLifecycle::Snapshot,
        AttachedActivityLifecycle::Missing(_) | AttachedActivityLifecycle::Invalid(_) => {
            HirActivityLifecycle::Stateless
        }
    }
}
