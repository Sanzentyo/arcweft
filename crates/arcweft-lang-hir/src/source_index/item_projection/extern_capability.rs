//! External-capability payload and source-owner freeze validation.

use arcweft_lang_syntax::attachment::{
    AttachedCapabilityAssociatedType, AttachedCapabilityFunction, AttachedCapabilityMember,
    AttachedExternCapabilityDeclaration, AttachedTypeFamily,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::identity::{ItemId, ScopeId};
use crate::item::{
    HirCapabilityAssociatedType, HirCapabilityFunction, HirCapabilityMember,
    HirDeclarationMemberArena, HirGenericParameter, HirItem, HirItemIssue, HirItemKind,
};
use crate::scope::HirPatternBindingPolicy;
use crate::slot::SlotSnapshot;
use crate::source_index::block_projection::BlockValidationArenas;

use super::callable::{
    function_parameter_groups_match, item_callable_scope_matches,
    item_owned_callable_scopes_are_exact, scope_children_are_exact_in_source_order,
};
use super::{
    ItemValidationArenas, expression_owner_matches, generic_issue, generic_parameters_match,
    item_prefix_matches, item_state, name_issue, prefix_issue, prefix_matches,
    required_name_matches, slot_is_poisoned, type_owner_matches,
};

struct CapabilityValidationContext<'context, 'arena> {
    owner: ItemId,
    item_scope: ScopeId,
    slots: &'context SlotSnapshot,
    arenas: &'context ItemValidationArenas<'arena>,
    block_arenas: BlockValidationArenas<'arena>,
}

struct MatchedCapabilityMember {
    issue: Option<HirItemIssue>,
}

pub(super) fn payload_matches(
    owner: ItemId,
    attached: &AttachedExternCapabilityDeclaration,
    item: &HirItem,
    declaration_members: Option<&HirDeclarationMemberArena>,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::ExternCapability(retained) = item.kind() else {
        return false;
    };
    if declaration_members.is_some()
        || !item.members().is_empty()
        || !item_prefix_matches(item, attached.prefix(), slots)
        || !prefix_expression_scopes_match(item.prefix(), item.scope(), slots, arenas)
        || !required_name_matches(retained.name(), attached.name())
        || retained.members().len() != attached.body().members().len()
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
    let context = CapabilityValidationContext {
        owner,
        item_scope: item.scope(),
        slots,
        arenas,
        block_arenas,
    };
    let mut expected_issue = prefix_issue(attached.prefix(), item.prefix(), slots)
        .or_else(|| name_issue(attached.name()))
        .or_else(|| {
            (!attached.header_recovery().is_empty()).then_some(HirItemIssue::MalformedHeader)
        })
        .or_else(|| {
            attached
                .body()
                .is_missing()
                .then_some(HirItemIssue::MissingBody)
        });
    let mut callable_scopes = Vec::new();
    for (position, (attached_member, retained_member)) in attached
        .body()
        .members()
        .iter()
        .zip(retained.members())
        .enumerate()
    {
        if usize::from(attached_member.source_ordinal()) != position {
            return false;
        }
        let Some(matched) = member_matches(
            &context,
            attached_member,
            retained_member,
            &mut callable_scopes,
        ) else {
            return false;
        };
        expected_issue = expected_issue.or(matched.issue);
    }
    expected_issue = expected_issue.or_else(|| {
        attached
            .body()
            .is_unclosed()
            .then_some(HirItemIssue::Recovery)
    });

    item_owned_callable_scopes_are_exact(owner, item.scope(), &callable_scopes, slots, arenas)
        && scope_children_are_exact_in_source_order(item.scope(), slots, arenas)
        && item.state() == &item_state(expected_issue)
}

fn member_matches(
    context: &CapabilityValidationContext<'_, '_>,
    attached: &AttachedCapabilityMember,
    retained: &HirCapabilityMember,
    callable_scopes: &mut Vec<ScopeId>,
) -> Option<MatchedCapabilityMember> {
    match (attached, retained) {
        (
            AttachedCapabilityMember::AssociatedType(attached),
            HirCapabilityMember::AssociatedType(retained),
        ) => associated_type_matches(
            attached,
            retained,
            context.item_scope,
            context.slots,
            context.arenas,
        ),
        (AttachedCapabilityMember::Function(attached), HirCapabilityMember::Function(retained)) => {
            let issue = function_matches(context, attached, retained)?;
            callable_scopes.push(retained.callable_scope());
            Some(issue)
        }
        (AttachedCapabilityMember::Error { .. }, HirCapabilityMember::Error) => {
            Some(MatchedCapabilityMember {
                issue: Some(HirItemIssue::InvalidMember),
            })
        }
        _ => None,
    }
}

fn associated_type_matches(
    attached: &AttachedCapabilityAssociatedType,
    retained: &HirCapabilityAssociatedType,
    item_scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<MatchedCapabilityMember> {
    if !prefix_matches(retained.prefix(), attached.prefix(), slots)
        || !prefix_expression_scopes_match(retained.prefix(), item_scope, slots, arenas)
        || !required_name_matches(retained.name(), attached.name())
        || !generic_parameters_match(retained.generic_parameters(), attached.generics(), slots)
        || !generic_parameter_scopes_match(retained.generic_parameters(), item_scope, slots, arenas)
    {
        return None;
    }
    let (missing_type, type_recovery) = match (attached.value(), retained.value()) {
        (None, None) => (false, false),
        (Some(attached), Some(retained)) => {
            if !type_owner_matches(retained, attached, slots)
                || !arenas
                    .types
                    .resolve_prepared(slots, retained)
                    .is_ok_and(|payload| payload.scope() == item_scope)
            {
                return None;
            }
            let recovered = slot_is_poisoned(slots, retained);
            (
                recovered
                    && attached.family() == AttachedTypeFamily::Recovery
                    && attached.syntax().kind() == SyntaxKind::MissingType,
                recovered,
            )
        }
        _ => return None,
    };
    Some(MatchedCapabilityMember {
        issue: prefix_issue(attached.prefix(), retained.prefix(), slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| {
                generic_issue(attached.generics(), retained.generic_parameters(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| missing_type.then_some(HirItemIssue::MissingType))
            .or_else(|| type_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            }),
    })
}

fn function_matches(
    context: &CapabilityValidationContext<'_, '_>,
    attached: &AttachedCapabilityFunction,
    retained: &HirCapabilityFunction,
) -> Option<MatchedCapabilityMember> {
    let callable_scope = retained.callable_scope();
    if !prefix_matches(retained.prefix(), attached.prefix(), context.slots)
        || !prefix_expression_scopes_match(
            retained.prefix(),
            context.item_scope,
            context.slots,
            context.arenas,
        )
        || !required_name_matches(retained.name(), attached.name())
        || !generic_parameters_match(
            retained.generic_parameters(),
            attached.generics(),
            context.slots,
        )
        || !generic_parameter_scopes_match(
            retained.generic_parameters(),
            callable_scope,
            context.slots,
            context.arenas,
        )
        || !item_callable_scope_matches(
            context.owner,
            context.item_scope,
            callable_scope,
            attached.syntax().id(),
            &attached.syntax().source_span(),
            context.slots,
            context.arenas,
        )
        || !scope_children_are_exact_in_source_order(callable_scope, context.slots, context.arenas)
    {
        return None;
    }

    let mut parameter_state = function_parameter_groups_match(
        attached.parameter_groups(),
        retained.parameter_groups(),
        callable_scope,
        HirPatternBindingPolicy::CallableParameter,
        context.slots,
        context.arenas,
        &context.block_arenas,
    )?;
    parameter_state.recovered |= attached.has_parameter_shape_recovery();
    let (return_missing_type, return_recovery) =
        capability_return_matches(attached, retained, context.slots, context.arenas)?;
    let effects_recovery =
        capability_effects_match(attached, retained, context.slots, context.arenas)?;

    Some(MatchedCapabilityMember {
        issue: prefix_issue(attached.prefix(), retained.prefix(), context.slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| {
                generic_issue(
                    attached.generics(),
                    retained.generic_parameters(),
                    context.slots,
                )
                .is_some()
                .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                parameter_state
                    .missing_type
                    .then_some(HirItemIssue::MissingType)
            })
            .or_else(|| {
                parameter_state
                    .recovered
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| return_missing_type.then_some(HirItemIssue::MissingType))
            .or_else(|| return_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| effects_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            }),
    })
}

fn capability_return_matches(
    attached: &AttachedCapabilityFunction,
    retained: &HirCapabilityFunction,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<(bool, bool)> {
    match (attached.authored_return(), retained.return_type()) {
        (None, None) => Some((false, false)),
        (Some(attached), Some(retained_type)) => {
            if !type_owner_matches(retained_type, attached.ty(), slots)
                || !arenas
                    .types
                    .resolve_prepared(slots, retained_type)
                    .is_ok_and(|payload| payload.scope() == retained.callable_scope())
            {
                return None;
            }
            let recovered = attached.has_recovery() || slot_is_poisoned(slots, retained_type);
            let missing = recovered
                && attached.ty().family() == AttachedTypeFamily::Recovery
                && attached.ty().syntax().kind() == SyntaxKind::MissingType;
            Some((missing, recovered))
        }
        _ => None,
    }
}

fn capability_effects_match(
    attached: &AttachedCapabilityFunction,
    retained_function: &HirCapabilityFunction,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    let Some(attached) = attached.effects() else {
        return retained_function.effects().is_empty().then_some(false);
    };
    if attached.expressions().len() != retained_function.effects().len() {
        return None;
    }
    let mut recovered = attached.has_recovery();
    for (attached, retained) in attached
        .expressions()
        .iter()
        .zip(retained_function.effects())
    {
        if !expression_owner_matches(
            *retained,
            attached,
            retained_function.callable_scope(),
            slots,
            arenas,
        ) {
            return None;
        }
        recovered |= slot_is_poisoned(slots, *retained);
    }
    Some(recovered)
}

fn generic_parameter_scopes_match(
    parameters: &[HirGenericParameter],
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    parameters.iter().all(|parameter| {
        parameter.bounds().iter().copied().all(|bound| {
            arenas
                .types
                .resolve_prepared(slots, bound)
                .is_ok_and(|payload| payload.scope() == scope)
        })
    })
}

fn prefix_expression_scopes_match(
    prefix: &crate::item::HirItemPrefix,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    prefix.attributes().iter().all(|attribute| {
        attribute.arguments().iter().all(|argument| {
            arenas
                .expressions
                .resolve_prepared(slots, argument.value())
                .is_ok_and(|payload| payload.scope() == scope)
        })
    })
}
