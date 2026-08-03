//! View-specific payload re-derivation for final item publication.

use arcweft_lang_syntax::attachment::{
    AttachedCallableParameterKind, AttachedViewDeclaration, AttachedViewPartPath,
};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::patterns::PatternSyntaxFamily;

use crate::identity::ItemId;
use crate::item::{
    HirDeclarationMemberArena, HirDeclarationMemberId, HirDeclarationMemberIssue,
    HirDeclarationMemberKind, HirDeclarationMemberPoisonState, HirItem, HirItemFamily,
    HirItemIssue, HirItemKind, HirParameter,
};
use crate::leaf::{HirPathIssue, HirPathRoot, HirPathValue};
use crate::scope::{HirPatternBindingPolicy, HirScopeKind, HirScopeOwner};
use crate::source_index::HirSourceSite;

use super::callable::{ParameterSurfacePolicy, parameters_match};
use super::{
    ItemValidationArenas, expression_owner_matches, item_prefix_matches, item_state, prefix_issue,
    retained_header_item_issue, retained_header_matches, slot_is_poisoned,
};
use crate::source_index::block_projection::BlockValidationArenas;
use crate::source_index::expression_manifest::leaf::path_projection_matches;

pub(super) fn payload_matches(
    owner: ItemId,
    attached: &AttachedViewDeclaration,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::View(view) = item.kind() else {
        return false;
    };
    let callable_scope = view.callable_scope();
    let Ok(callable) = arenas.scopes.resolve_prepared(slots, callable_scope) else {
        return false;
    };
    let Ok(module_scope) = arenas.scopes.resolve_prepared(slots, item.scope()) else {
        return false;
    };
    let callable_source_matches = slots
        .resolve_prepared(callable_scope)
        .is_ok_and(|metadata| {
            metadata.source_site() == &HirSourceSite::Span(attached.syntax().source_span())
                && matches!(
                    metadata.origin(),
                    crate::slot::HirOrigin::Source(source)
                        if source.syntax() == attached.syntax().id()
                )
        });
    if !item_prefix_matches(item, attached.prefix(), slots)
        || !retained_header_matches(view.header(), attached.header())
        || !callable_source_matches
        || callable.kind() != HirScopeKind::Callable
        || callable.parent() != Some(item.scope())
        || callable.owner() != &HirScopeOwner::Item(owner)
        || !module_scope.children().contains(&callable_scope)
        || attached.parameter_group().source_ordinal() != 0
        || attached
            .parameter_group()
            .parameters()
            .iter()
            .enumerate()
            .any(|(position, parameter)| {
                u16::try_from(position).map_or(true, |position| {
                    parameter.source_ordinal() != position
                        || parameter.group_ordinal() != 0
                        || parameter.parameter_ordinal() != position
                })
            })
    {
        return false;
    }

    let block_arenas = BlockValidationArenas {
        expressions: arenas.expressions,
        statements: arenas.statements,
        scopes: arenas.scopes,
        locals: arenas.locals,
        patterns: arenas.patterns,
    };
    if parameters_match(
        attached.parameter_group().parameters(),
        attached.parameter_group().has_recovery(),
        ParameterSurfacePolicy::FixedWithDefaults,
        view.parameters(),
        callable_scope,
        HirPatternBindingPolicy::CallableParameter,
        slots,
        arenas,
        &block_arenas,
    )
    .is_none()
    {
        return false;
    }

    let retained_members = match members {
        Some(members) if members.owner() == owner && members.family() == HirItemFamily::View => {
            members.members()
        }
        Some(_) => return false,
        None => &[],
    };
    let attached_exports = attached.exports().collect::<Vec<_>>();
    if attached_exports.len() != retained_members.len()
        || attached_exports.len() != item.members().len()
        || attached_exports.len() != view.exports().len()
    {
        return false;
    }
    let mut member_issue = None;
    for (position, (attached_export, retained)) in
        attached_exports.iter().zip(retained_members).enumerate()
    {
        let Ok(ordinal) = u32::try_from(position) else {
            return false;
        };
        let id = HirDeclarationMemberId::new(owner, ordinal);
        let HirDeclarationMemberKind::ViewExport(export) = retained.kind() else {
            return false;
        };
        let expected_state = if attached_export.has_recovery() || export.has_recovery() {
            member_issue.get_or_insert(HirItemIssue::InvalidMember);
            HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
        } else {
            HirDeclarationMemberPoisonState::Clean
        };
        if retained.id() != id
            || usize::from(attached_export.source_ordinal()) != position
            || item.members()[position] != id
            || view.exports()[position] != id
            || !view_part_matches(export.local_part(), attached_export.local_part())
            || !view_part_matches(export.public_part(), attached_export.public_part())
            || retained.state() != expected_state
        {
            return false;
        }
    }
    if (retained_members.is_empty() && members.is_some())
        || (!retained_members.is_empty() && members.is_none())
    {
        return false;
    }

    let attached_values = attached
        .body()
        .fragment()
        .map(|fragment| fragment.values().collect::<Vec<_>>())
        .unwrap_or_default();
    if attached_values.len() != view.values().len() {
        return false;
    }
    let mut value_issue = None;
    for (attached_value, retained) in attached_values.into_iter().zip(view.values()) {
        if !expression_owner_matches(*retained, attached_value, callable_scope, slots, arenas) {
            return false;
        }
        if slot_is_poisoned(slots, *retained) {
            value_issue.get_or_insert(HirItemIssue::InvalidMember);
        }
    }

    let parameter_issue = first_parameter_issue(attached, view.parameters(), slots);
    let expected_state = item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| retained_header_item_issue(attached.header()))
            .or_else(|| {
                attached
                    .header_recovery()
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                (attached.parameter_group().open_state().is_missing()
                    || attached.parameter_group().close_state().is_missing())
                .then_some(HirItemIssue::Recovery)
            })
            .or(parameter_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_missing()
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(member_issue)
            .or(value_issue)
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
    );
    item.state() == &expected_state
}

fn view_part_matches(retained: &HirPathValue, attached: &AttachedViewPartPath) -> bool {
    match attached {
        AttachedViewPartPath::Path(attached) => path_projection_matches(retained, attached),
        AttachedViewPartPath::Missing(_) => matches!(
            retained,
            HirPathValue::Recovered(recovery)
                if recovery.root() == HirPathRoot::ImplicitCrate
                    && recovery.segment_count() == 0
                    && recovery.issue() == &HirPathIssue::Empty
        ),
    }
}

fn first_parameter_issue(
    attached: &AttachedViewDeclaration,
    retained: &[HirParameter],
    slots: &crate::slot::SlotSnapshot,
) -> Option<HirItemIssue> {
    attached
        .parameter_group()
        .parameters()
        .iter()
        .zip(retained)
        .find_map(|(attached, retained)| {
            let pattern_poisoned = slot_is_poisoned(slots, retained.pattern());
            let type_poisoned = slot_is_poisoned(slots, retained.ty());
            let default_poisoned = retained
                .default()
                .is_some_and(|default| slot_is_poisoned(slots, default));
            let invalid_shape = !matches!(attached.kind(), AttachedCallableParameterKind::Fixed)
                || attached.pattern().family() != PatternSyntaxFamily::Binding;
            invalid_shape
                .then_some(HirItemIssue::InvalidMember)
                .or_else(|| {
                    (type_poisoned && attached.ty().syntax().kind() == SyntaxKind::MissingType)
                        .then_some(HirItemIssue::MissingType)
                })
                .or_else(|| {
                    (attached.has_recovery()
                        || pattern_poisoned
                        || type_poisoned
                        || default_poisoned)
                        .then_some(HirItemIssue::InvalidMember)
                })
        })
}
