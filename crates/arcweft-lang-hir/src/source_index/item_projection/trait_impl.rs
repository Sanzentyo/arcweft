//! Trait/Impl inline-member re-derivation for final item publication.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedAttributeValue, AttachedCallableParameterKind, AttachedCallableReturn,
    AttachedFunctionBody, AttachedGenericParameterGroup, AttachedImplAssociatedType,
    AttachedImplDeclaration, AttachedImplFunction, AttachedImplMember, AttachedItemPrefix,
    AttachedMethodParameter, AttachedMethodParameterGroup, AttachedMethodReceiverKind,
    AttachedRequiredName, AttachedTraitAssociatedType, AttachedTraitDeclaration,
    AttachedTraitFunction, AttachedTraitMember, AttachedTypeFamily, AttachedWhereClause,
};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::identity::{
    ExprId, ItemId, LocalGeneration, LocalId, ScopeId, SyntheticOwner, SyntheticRole,
};
use crate::item::{
    HirDeclarationMemberArena, HirFunctionBody, HirGenericParameter, HirImplAssociatedType,
    HirImplFunction, HirImplMember, HirItem, HirItemIssue, HirItemKind, HirMethodParameter,
    HirMethodParameterGroup, HirMethodReceiverKind, HirParameterKind, HirRequiredName,
    HirTraitAssociatedType, HirTraitFunction, HirTraitMember, HirWherePredicate,
};
use crate::leaf::HirName;
use crate::scope::HirPatternBindingPolicy;
use crate::slot::{HirOrigin, SlotSnapshot};

use super::callable::{
    item_callable_scope_matches, item_owned_callable_scopes_are_exact,
    scope_children_are_exact_in_source_order, scope_locals_are_exact,
};
use super::{
    ItemValidationArenas, expression_owner_matches, generic_issue, generic_parameters_match,
    item_state, name_issue, prefix_issue, prefix_matches, required_name_matches, slot_is_poisoned,
    type_owner_matches, where_issue, where_predicates_match,
};
use crate::source_index::block_projection::{
    BlockValidationArenas, MethodValueBlockRetained, method_block_matches, source_owner_matches,
};
use crate::source_index::control_projection::canonical_pattern_locals;
use crate::source_index::pattern_projection::{BindingLocalValidation, binding_locals_match};

pub(super) fn trait_payload_matches(
    owner: ItemId,
    attached: &AttachedTraitDeclaration,
    item: &HirItem,
    member_arena: Option<&HirDeclarationMemberArena>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Trait(retained) = item.kind() else {
        return false;
    };
    if member_arena.is_some()
        || !item.members().is_empty()
        || !prefix_matches(item.prefix(), attached.prefix(), slots)
        || !prefix_expression_scopes_match(attached.prefix(), item.scope(), slots, arenas)
        || !required_name_matches(retained.name(), attached.name())
        || !generic_parameters_match(retained.generic_parameters(), attached.generics(), slots)
        || !generic_types_are_in_scope(retained.generic_parameters(), item.scope(), slots, arenas)
        || !types_match(
            retained.supertraits(),
            attached.supertraits(),
            item.scope(),
            slots,
            arenas,
        )
        || !where_predicates_match(retained.where_predicates(), attached.where_clauses(), slots)
        || !where_types_are_in_scope(retained.where_predicates(), item.scope(), slots, arenas)
        || retained.members().len() != attached.body().members().len()
    {
        return false;
    }

    let mut first_member_issue = None;
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
        let Some(evidence) = trait_member_matches(
            owner,
            item.scope(),
            attached_member,
            retained_member,
            parsed,
            slots,
            arenas,
        ) else {
            return false;
        };
        first_member_issue = first_member_issue.or(evidence.issue);
        callable_scopes.extend(evidence.callable_scope);
    }

    let expected = item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| {
                generic_issue(attached.generics(), retained.generic_parameters(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                retained
                    .supertraits()
                    .iter()
                    .copied()
                    .any(|ty| slot_is_poisoned(slots, ty))
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                where_issue(attached.where_clauses(), retained.where_predicates(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .body()
                    .is_missing()
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(first_member_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            }),
    );
    item_owned_callable_scopes_are_exact(owner, item.scope(), &callable_scopes, slots, arenas)
        && scope_children_are_exact_in_source_order(item.scope(), slots, arenas)
        && item.state() == &expected
}

pub(super) fn impl_payload_matches(
    owner: ItemId,
    attached: &AttachedImplDeclaration,
    item: &HirItem,
    member_arena: Option<&HirDeclarationMemberArena>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Impl(retained) = item.kind() else {
        return false;
    };
    if member_arena.is_some()
        || !item.members().is_empty()
        || !prefix_matches(item.prefix(), attached.prefix(), slots)
        || !prefix_expression_scopes_match(attached.prefix(), item.scope(), slots, arenas)
        || !generic_parameters_match(retained.generic_parameters(), attached.generics(), slots)
        || !generic_types_are_in_scope(retained.generic_parameters(), item.scope(), slots, arenas)
        || !optional_type_matches(
            retained.trait_ref(),
            attached.trait_ref(),
            item.scope(),
            slots,
            arenas,
        )
        || !type_matches(
            retained.target(),
            attached.target(),
            item.scope(),
            slots,
            arenas,
        )
        || !where_predicates_match(retained.where_predicates(), attached.where_clauses(), slots)
        || !where_types_are_in_scope(retained.where_predicates(), item.scope(), slots, arenas)
        || retained.members().len() != attached.body().members().len()
    {
        return false;
    }

    let mut first_member_issue = None;
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
        let Some(evidence) = impl_member_matches(
            owner,
            item.scope(),
            attached_member,
            retained_member,
            parsed,
            slots,
            arenas,
        ) else {
            return false;
        };
        first_member_issue = first_member_issue.or(evidence.issue);
        callable_scopes.extend(evidence.callable_scope);
    }

    let target_poisoned = slot_is_poisoned(slots, retained.target());
    let target_missing =
        target_poisoned && attached.target().syntax().kind() == SyntaxKind::MissingType;
    let trait_poisoned = retained
        .trait_ref()
        .is_some_and(|ty| slot_is_poisoned(slots, ty));
    let expected = item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| {
                generic_issue(attached.generics(), retained.generic_parameters(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| target_missing.then_some(HirItemIssue::MissingType))
            .or_else(|| {
                (trait_poisoned || target_poisoned).then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                where_issue(attached.where_clauses(), retained.where_predicates(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .body()
                    .is_missing()
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(first_member_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            }),
    );
    item_owned_callable_scopes_are_exact(owner, item.scope(), &callable_scopes, slots, arenas)
        && scope_children_are_exact_in_source_order(item.scope(), slots, arenas)
        && item.state() == &expected
}

#[derive(Clone, Copy)]
struct MemberEvidence {
    issue: Option<HirItemIssue>,
    callable_scope: Option<ScopeId>,
}

fn trait_member_matches(
    owner: ItemId,
    item_scope: ScopeId,
    attached: &AttachedTraitMember,
    retained: &HirTraitMember,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<MemberEvidence> {
    match (attached, retained) {
        (
            AttachedTraitMember::AssociatedType(attached),
            HirTraitMember::AssociatedType(retained),
        ) => trait_associated_type_matches(attached, retained, item_scope, slots, arenas),
        (AttachedTraitMember::Function(attached), HirTraitMember::Function(retained)) => {
            method_matches(
                owner,
                item_scope,
                MethodAttachment::from(attached),
                MethodRetention::from(retained),
                parsed,
                slots,
                arenas,
            )
        }
        (AttachedTraitMember::Error { .. }, HirTraitMember::Error) => Some(MemberEvidence {
            issue: Some(HirItemIssue::InvalidMember),
            callable_scope: None,
        }),
        _ => None,
    }
}

fn impl_member_matches(
    owner: ItemId,
    item_scope: ScopeId,
    attached: &AttachedImplMember,
    retained: &HirImplMember,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<MemberEvidence> {
    match (attached, retained) {
        (AttachedImplMember::AssociatedType(attached), HirImplMember::AssociatedType(retained)) => {
            impl_associated_type_matches(attached, retained, item_scope, slots, arenas)
        }
        (AttachedImplMember::Function(attached), HirImplMember::Function(retained)) => {
            method_matches(
                owner,
                item_scope,
                MethodAttachment::from(attached),
                MethodRetention::from(retained),
                parsed,
                slots,
                arenas,
            )
        }
        (AttachedImplMember::Error { .. }, HirImplMember::Error) => Some(MemberEvidence {
            issue: Some(HirItemIssue::InvalidMember),
            callable_scope: None,
        }),
        _ => None,
    }
}

fn trait_associated_type_matches(
    attached: &AttachedTraitAssociatedType,
    retained: &HirTraitAssociatedType,
    item_scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<MemberEvidence> {
    if !prefix_matches(retained.prefix(), attached.prefix(), slots)
        || !prefix_expression_scopes_match(attached.prefix(), item_scope, slots, arenas)
        || !required_name_matches(retained.name(), attached.name())
        || !generic_parameters_match(retained.generic_parameters(), attached.generics(), slots)
        || !generic_types_are_in_scope(retained.generic_parameters(), item_scope, slots, arenas)
        || !optional_type_matches(
            retained.default(),
            attached.default(),
            item_scope,
            slots,
            arenas,
        )
    {
        return None;
    }
    let default_recovery = retained
        .default()
        .is_some_and(|ty| slot_is_poisoned(slots, ty));
    Some(MemberEvidence {
        issue: prefix_issue(attached.prefix(), retained.prefix(), slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| {
                generic_issue(attached.generics(), retained.generic_parameters(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| default_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            }),
        callable_scope: None,
    })
}

fn impl_associated_type_matches(
    attached: &AttachedImplAssociatedType,
    retained: &HirImplAssociatedType,
    item_scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<MemberEvidence> {
    if !prefix_matches(retained.prefix(), attached.prefix(), slots)
        || !prefix_expression_scopes_match(attached.prefix(), item_scope, slots, arenas)
        || !required_name_matches(retained.name(), attached.name())
        || !generic_parameters_match(retained.generic_parameters(), attached.generics(), slots)
        || !generic_types_are_in_scope(retained.generic_parameters(), item_scope, slots, arenas)
        || !type_matches(
            retained.target(),
            attached.target(),
            item_scope,
            slots,
            arenas,
        )
    {
        return None;
    }
    let target_recovery = slot_is_poisoned(slots, retained.target());
    let target_missing =
        target_recovery && attached.target().syntax().kind() == SyntaxKind::MissingType;
    Some(MemberEvidence {
        issue: prefix_issue(attached.prefix(), retained.prefix(), slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| {
                generic_issue(attached.generics(), retained.generic_parameters(), slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| target_missing.then_some(HirItemIssue::MissingType))
            .or_else(|| target_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            }),
        callable_scope: None,
    })
}

struct MethodAttachment<'a> {
    syntax: arcweft_lang_syntax::attachment::SyntaxNodeId,
    source: arcweft_source::SourceSpan,
    prefix: &'a AttachedItemPrefix,
    name: &'a AttachedRequiredName,
    generics: Option<&'a AttachedGenericParameterGroup>,
    parameter_groups: &'a [AttachedMethodParameterGroup],
    parameter_shape_recovery: bool,
    where_clauses: &'a [AttachedWhereClause],
    authored_return: Option<&'a AttachedCallableReturn>,
    body: Option<&'a AttachedFunctionBody>,
    trailing_recovery: bool,
}

impl<'a> From<&'a AttachedTraitFunction> for MethodAttachment<'a> {
    fn from(value: &'a AttachedTraitFunction) -> Self {
        Self {
            syntax: value.syntax().id(),
            source: value.syntax().source_span(),
            prefix: value.prefix(),
            name: value.name(),
            generics: value.generics(),
            parameter_groups: value.parameter_groups(),
            parameter_shape_recovery: value.has_parameter_shape_recovery(),
            where_clauses: value.where_clauses(),
            authored_return: value.authored_return(),
            body: value.body(),
            trailing_recovery: !value.trailing_recovery().is_empty(),
        }
    }
}

impl<'a> From<&'a AttachedImplFunction> for MethodAttachment<'a> {
    fn from(value: &'a AttachedImplFunction) -> Self {
        Self {
            syntax: value.syntax().id(),
            source: value.syntax().source_span(),
            prefix: value.prefix(),
            name: value.name(),
            generics: value.generics(),
            parameter_groups: value.parameter_groups(),
            parameter_shape_recovery: value.has_parameter_shape_recovery(),
            where_clauses: value.where_clauses(),
            authored_return: value.authored_return(),
            body: value.body(),
            trailing_recovery: !value.trailing_recovery().is_empty(),
        }
    }
}

struct MethodRetention<'a> {
    prefix: &'a crate::item::HirItemPrefix,
    name: &'a HirRequiredName,
    generic_parameters: &'a [HirGenericParameter],
    parameter_groups: &'a [HirMethodParameterGroup],
    where_predicates: &'a [HirWherePredicate],
    return_type: Option<crate::identity::TypeId>,
    callable_scope: ScopeId,
    body: Option<&'a HirFunctionBody>,
}

impl<'a> From<&'a HirTraitFunction> for MethodRetention<'a> {
    fn from(value: &'a HirTraitFunction) -> Self {
        Self {
            prefix: value.prefix(),
            name: value.name(),
            generic_parameters: value.generic_parameters(),
            parameter_groups: value.parameter_groups(),
            where_predicates: value.where_predicates(),
            return_type: value.return_type(),
            callable_scope: value.callable_scope(),
            body: value.body(),
        }
    }
}

impl<'a> From<&'a HirImplFunction> for MethodRetention<'a> {
    fn from(value: &'a HirImplFunction) -> Self {
        Self {
            prefix: value.prefix(),
            name: value.name(),
            generic_parameters: value.generic_parameters(),
            parameter_groups: value.parameter_groups(),
            where_predicates: value.where_predicates(),
            return_type: value.return_type(),
            callable_scope: value.callable_scope(),
            body: value.body(),
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "one method projection compares the closed attached and retained schemas plus all owned types, scopes, and bodies"
)]
fn method_matches(
    owner: ItemId,
    item_scope: ScopeId,
    attached: MethodAttachment<'_>,
    retained: MethodRetention<'_>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<MemberEvidence> {
    if !prefix_matches(retained.prefix, attached.prefix, slots)
        || !prefix_expression_scopes_match(attached.prefix, item_scope, slots, arenas)
        || !required_name_matches(retained.name, attached.name)
        || !generic_parameters_match(retained.generic_parameters, attached.generics, slots)
        || !generic_types_are_in_scope(
            retained.generic_parameters,
            retained.callable_scope,
            slots,
            arenas,
        )
        || !where_predicates_match(retained.where_predicates, attached.where_clauses, slots)
        || !where_types_are_in_scope(
            retained.where_predicates,
            retained.callable_scope,
            slots,
            arenas,
        )
        || !item_callable_scope_matches(
            owner,
            item_scope,
            retained.callable_scope,
            attached.syntax,
            &attached.source,
            slots,
            arenas,
        )
    {
        return None;
    }

    let block_arenas = block_arenas(arenas);
    let mut parameter = method_parameter_groups_match(
        attached.parameter_groups,
        retained.parameter_groups,
        retained.callable_scope,
        slots,
        arenas,
        &block_arenas,
    )?;
    parameter.recovered |= attached.parameter_shape_recovery;
    let (return_missing, return_recovered) = method_return_matches(
        attached.authored_return,
        retained.return_type,
        retained.callable_scope,
        slots,
        arenas,
    )?;
    let body_recovered = match (attached.body, retained.body) {
        (
            Some(attached_body @ AttachedFunctionBody::Block { block, .. }),
            Some(HirFunctionBody::Block {
                scope,
                statements,
                tail,
            }),
        ) if *scope == retained.callable_scope => {
            let recovered = method_block_matches(
                parsed,
                slots,
                &block_arenas,
                MethodValueBlockRetained {
                    owner,
                    scope: retained.callable_scope,
                    parameter_locals: &parameter.locals,
                    statements,
                    tail: *tail,
                },
                block,
                &mut parameter.generations,
            )?;
            attached_body.has_recovery() || recovered
        }
        (None, None) => {
            if !bodyless_scope_matches(retained.callable_scope, &parameter.locals, slots, arenas) {
                return None;
            }
            false
        }
        _ => return None,
    };
    if !scope_children_are_exact_in_source_order(retained.callable_scope, slots, arenas) {
        return None;
    }

    Some(MemberEvidence {
        issue: prefix_issue(attached.prefix, retained.prefix, slots)
            .or_else(|| name_issue(attached.name))
            .or_else(|| {
                generic_issue(attached.generics, retained.generic_parameters, slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| parameter.missing_type.then_some(HirItemIssue::MissingType))
            .or_else(|| parameter.recovered.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| return_missing.then_some(HirItemIssue::MissingType))
            .or_else(|| return_recovered.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| {
                where_issue(attached.where_clauses, retained.where_predicates, slots)
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| body_recovered.then_some(HirItemIssue::Recovery))
            .or_else(|| attached.trailing_recovery.then_some(HirItemIssue::Recovery)),
        callable_scope: Some(retained.callable_scope),
    })
}

struct MethodParameterState {
    locals: Box<[LocalId]>,
    generations: BTreeMap<HirName, LocalGeneration>,
    missing_type: bool,
    recovered: bool,
}

#[allow(
    clippy::too_many_lines,
    reason = "one ordered matrix validates every method parameter-group form and its typed binding ownership"
)]
fn method_parameter_groups_match(
    attached: &[AttachedMethodParameterGroup],
    retained: &[HirMethodParameterGroup],
    callable_scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
    block_arenas: &BlockValidationArenas<'_>,
) -> Option<MethodParameterState> {
    if attached.len() != retained.len() || attached.is_empty() {
        return None;
    }
    let mut locals = Vec::new();
    let mut generations = BTreeMap::new();
    let mut missing_type = false;
    let mut recovered = false;
    let mut source_position = 0_usize;
    for (group_position, (attached_group, retained_group)) in
        attached.iter().zip(retained).enumerate()
    {
        if usize::from(attached_group.source_ordinal()) != group_position
            || attached_group.parameters().len() != retained_group.parameters().len()
        {
            return None;
        }
        recovered |= attached_group.has_recovery();
        for (parameter_position, (attached_parameter, retained_parameter)) in attached_group
            .parameters()
            .iter()
            .zip(retained_group.parameters())
            .enumerate()
        {
            if usize::from(attached_parameter.source_ordinal()) != source_position
                || usize::from(attached_parameter.group_ordinal()) != group_position
                || usize::from(attached_parameter.parameter_ordinal()) != parameter_position
            {
                return None;
            }
            source_position += 1;
            match (attached_parameter, retained_parameter) {
                (
                    AttachedMethodParameter::Receiver(attached),
                    HirMethodParameter::Receiver(retained),
                ) => {
                    if retained.kind() != receiver_kind(attached.kind())
                        || !source_owner_matches(
                            slots,
                            retained.pattern(),
                            attached.pattern().id(),
                            &crate::source_index::HirSourceSite::Span(
                                attached.pattern().whole_source_span(),
                            ),
                        )
                    {
                        return None;
                    }
                    let pattern = arenas
                        .patterns
                        .resolve_prepared(slots, retained.pattern())
                        .ok()?;
                    let expected = canonical_pattern_locals(
                        slots,
                        block_arenas,
                        retained.pattern(),
                        retained.pattern(),
                        callable_scope,
                    )?;
                    let expected_ids = expected.iter().map(|entry| entry.local).collect::<Vec<_>>();
                    let mut validation = BindingLocalValidation::new(
                        callable_scope,
                        HirPatternBindingPolicy::CallableParameter,
                        &mut generations,
                        slots,
                        arenas.patterns,
                        arenas.locals,
                    );
                    if pattern.scope() != callable_scope
                        || expected_ids.len() != 1
                        || retained.locals() != expected_ids
                        || !binding_locals_match(attached.pattern(), &expected, &mut validation)
                    {
                        return None;
                    }
                    recovered |= pattern.is_poisoned() || validation.is_poisoned();
                    locals.extend(expected_ids);
                }
                (AttachedMethodParameter::Typed(attached), HirMethodParameter::Typed(retained)) => {
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
                        (Some(attached), Some(retained)) => expression_owner_matches(
                            retained,
                            attached.value(),
                            callable_scope,
                            slots,
                            arenas,
                        ),
                        _ => false,
                    };
                    if !kind_matches
                        || !default_matches
                        || !type_matches(
                            retained.ty(),
                            attached.ty(),
                            callable_scope,
                            slots,
                            arenas,
                        )
                        || !source_owner_matches(
                            slots,
                            retained.pattern(),
                            attached.pattern().id(),
                            &crate::source_index::HirSourceSite::Span(
                                attached.pattern().whole_source_span(),
                            ),
                        )
                    {
                        return None;
                    }
                    let pattern = arenas
                        .patterns
                        .resolve_prepared(slots, retained.pattern())
                        .ok()?;
                    let ty = arenas.types.resolve_prepared(slots, retained.ty()).ok()?;
                    let expected = canonical_pattern_locals(
                        slots,
                        block_arenas,
                        retained.pattern(),
                        retained.pattern(),
                        callable_scope,
                    )?;
                    let expected_ids = expected.iter().map(|entry| entry.local).collect::<Vec<_>>();
                    let mut validation = BindingLocalValidation::new(
                        callable_scope,
                        HirPatternBindingPolicy::CallableParameter,
                        &mut generations,
                        slots,
                        arenas.patterns,
                        arenas.locals,
                    );
                    if pattern.scope() != callable_scope
                        || retained.locals() != expected_ids
                        || !binding_locals_match(attached.pattern(), &expected, &mut validation)
                    {
                        return None;
                    }
                    missing_type |= ty.is_poisoned()
                        && attached.ty().syntax().kind() == SyntaxKind::MissingType;
                    recovered |= pattern.is_poisoned()
                        || ty.is_poisoned()
                        || validation.is_poisoned()
                        || attached.has_recovery()
                        || retained
                            .default()
                            .is_some_and(|value| slot_is_poisoned(slots, value));
                    locals.extend(expected_ids);
                }
                _ => return None,
            }
        }
    }
    if locals.iter().copied().collect::<BTreeSet<_>>().len() != locals.len() {
        return None;
    }
    Some(MethodParameterState {
        locals: locals.into_boxed_slice(),
        generations,
        missing_type,
        recovered,
    })
}

fn method_return_matches(
    attached: Option<&AttachedCallableReturn>,
    retained: Option<crate::identity::TypeId>,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<(bool, bool)> {
    match (attached, retained) {
        (None, None) => Some((false, false)),
        (Some(attached), Some(retained)) => {
            if !type_matches(retained, attached.ty(), scope, slots, arenas) {
                return None;
            }
            let recovered = attached.has_recovery() || slot_is_poisoned(slots, retained);
            let missing = recovered
                && attached.ty().family() == AttachedTypeFamily::Recovery
                && attached.ty().syntax().kind() == SyntaxKind::MissingType;
            Some((missing, recovered))
        }
        _ => None,
    }
}

fn bodyless_scope_matches(
    scope: ScopeId,
    parameter_locals: &[LocalId],
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let no_statements = arenas
        .statements
        .try_iter_prepared(slots)
        .is_ok_and(|mut entries| entries.all(|(_, statement)| statement.scope() != scope));
    let no_tail = !slots.prepared_live_ids::<ExprId>().any(|expression| {
        slots.resolve_prepared(expression).is_ok_and(|metadata| {
            matches!(
                metadata.origin(),
                HirOrigin::Synthetic(key)
                    if key.owner() == SyntheticOwner::Scope(scope)
                        && matches!(
                            key.role(),
                            SyntheticRole::ImplicitUnitTail | SyntheticRole::MissingRequiredTail
                        )
            )
        })
    });
    arenas
        .scopes
        .resolve_prepared(slots, scope)
        .is_ok_and(|payload| payload.locals() == parameter_locals)
        && scope_locals_are_exact(scope, parameter_locals, slots, arenas)
        && no_statements
        && no_tail
}

fn prefix_expression_scopes_match(
    prefix: &AttachedItemPrefix,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    prefix.attributes().iter().all(|attribute| {
        attribute
            .arguments()
            .iter()
            .all(|argument| match argument.value() {
                AttachedAttributeValue::Authored(expression) => slots
                    .prepared_source_owner::<ExprId>(expression.id())
                    .is_some_and(|owner| {
                        expression_owner_matches(owner, expression, scope, slots, arenas)
                    }),
                AttachedAttributeValue::Missing(_) => true,
            })
    })
}

fn optional_type_matches(
    retained: Option<crate::identity::TypeId>,
    attached: Option<&arcweft_lang_syntax::attachment::AttachedTypeRefNode>,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    match (retained, attached) {
        (None, None) => true,
        (Some(retained), Some(attached)) => type_matches(retained, attached, scope, slots, arenas),
        _ => false,
    }
}

fn types_match(
    retained: &[crate::identity::TypeId],
    attached: &[arcweft_lang_syntax::attachment::AttachedTypeRefNode],
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    retained.len() == attached.len()
        && retained
            .iter()
            .copied()
            .zip(attached)
            .all(|(retained, attached)| type_matches(retained, attached, scope, slots, arenas))
}

fn type_matches(
    retained: crate::identity::TypeId,
    attached: &arcweft_lang_syntax::attachment::AttachedTypeRefNode,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    type_owner_matches(retained, attached, slots)
        && arenas
            .types
            .resolve_prepared(slots, retained)
            .is_ok_and(|ty| ty.scope() == scope)
}

fn generic_types_are_in_scope(
    retained: &[HirGenericParameter],
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    retained.iter().all(|parameter| {
        parameter.bounds().iter().copied().all(|ty| {
            arenas
                .types
                .resolve_prepared(slots, ty)
                .is_ok_and(|payload| payload.scope() == scope)
        })
    })
}

fn where_types_are_in_scope(
    retained: &[HirWherePredicate],
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    retained.iter().all(|predicate| {
        core::iter::once(predicate.subject())
            .chain(predicate.bounds().iter().copied())
            .all(|ty| {
                arenas
                    .types
                    .resolve_prepared(slots, ty)
                    .is_ok_and(|payload| payload.scope() == scope)
            })
    })
}

fn block_arenas<'arena>(arenas: &ItemValidationArenas<'arena>) -> BlockValidationArenas<'arena> {
    BlockValidationArenas {
        expressions: arenas.expressions,
        statements: arenas.statements,
        scopes: arenas.scopes,
        locals: arenas.locals,
        patterns: arenas.patterns,
    }
}

const fn receiver_kind(kind: AttachedMethodReceiverKind) -> HirMethodReceiverKind {
    match kind {
        AttachedMethodReceiverKind::Owned => HirMethodReceiverKind::Owned,
        AttachedMethodReceiverKind::SharedReference => HirMethodReceiverKind::SharedReference,
        AttachedMethodReceiverKind::MutableReference => HirMethodReceiverKind::MutableReference,
    }
}
