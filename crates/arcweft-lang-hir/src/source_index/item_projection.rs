//! Attached-item payload validation for the final HIR item arena.
//!
//! Item whole sites remain on their slots. Native Style additionally exposes
//! a closed item-owned component family through the sole source index; all
//! other item payloads are re-derived directly from accepted attached syntax.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedActionDeclaration, AttachedAttributeValue, AttachedCharacterBody,
    AttachedCharacterDeclaration, AttachedCharacterDisplayNameMember, AttachedCharacterInitializer,
    AttachedCharacterMember, AttachedCharacterSurfaceAlias, AttachedDeclarationPublicId,
    AttachedDeclarationPublicIdIssue, AttachedEnumBody, AttachedExpressionNode,
    AttachedGenericParameter, AttachedItemPrefix, AttachedNominalDeclaration,
    AttachedOuterAttribute, AttachedRequiredName, AttachedRetainedHeader, AttachedRetainedName,
    AttachedSignalDeclaration, AttachedStructBody, AttachedTypeAliasDeclaration,
    AttachedTypeRefNode, AttachedWhereClause, TypedItemNode,
};
use arcweft_lang_syntax::expressions::{
    SyntaxCallArgumentListTerminator, SyntaxCallArgumentProjection, SyntaxRequiredTokenState,
};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use super::block_projection::BlockValidationArenas;
use super::control_projection::canonical_pattern_locals;
use super::expression_manifest::leaf::{
    attached_path_is_resolved, resolved_path_projection_matches,
};
use super::pattern_projection::{BindingLocalValidation, binding_locals_match};
use super::{HirSourceIndex, HirSourceSite};
use crate::arena::ArenaSnapshot;
use crate::expr::{
    HirCallArgument, HirCallChildPoison, HirCallExpr, HirCallValue, HirExpr, HirRecoveredName,
    HirRequiredTokenState,
};
use crate::identity::{
    ExprId, HirTypedId, ItemId, LocalId, PatternId, ScopeId, StmtId, SyntheticOwner, TypeId,
};
use crate::item::{
    HirActionDeclaration, HirCharacterAssignmentState, HirCharacterSurfaceAlias,
    HirDeclarationMember, HirDeclarationMemberArena, HirDeclarationMemberId,
    HirDeclarationMemberIndex, HirDeclarationMemberIssue, HirDeclarationMemberKind,
    HirDeclarationMemberPoisonState, HirEnumItem, HirGenericParameter, HirItem, HirItemIssue,
    HirItemKind, HirItemPoisonState, HirPublicIdOrigin, HirRequiredName, HirRetainedName,
    HirRetainedPublicId, HirRetainedPublicIdIssue, HirStructItem, HirTypeAliasItem,
    HirWherePredicate,
};
use crate::leaf::HirName;
use crate::pattern::HirPattern;
use crate::scope::{
    HirLocal, HirLocalKind, HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner,
};
use crate::slot::{HirOrigin, HirSlotMetadata, SlotSnapshot};
use crate::stmt::HirStmt;
use crate::type_ref::HirType;

struct RetainedAttributeProjection<'a> {
    attached: &'a AttachedOuterAttribute,
    arguments: Box<[HirCallArgument]>,
}

pub(crate) struct ItemValidationArenas<'a> {
    pub(crate) scopes: &'a ArenaSnapshot<HirScope, ScopeId>,
    pub(crate) locals: &'a ArenaSnapshot<HirLocal, LocalId>,
    pub(crate) expressions: &'a ArenaSnapshot<HirExpr, ExprId>,
    pub(crate) statements: &'a ArenaSnapshot<HirStmt, StmtId>,
    pub(crate) patterns: &'a ArenaSnapshot<HirPattern, PatternId>,
    pub(crate) types: &'a ArenaSnapshot<HirType, TypeId>,
}

mod activity;
mod callable;
mod callable_source;
mod declaration;
mod entry;
mod entry_source;
mod extern_capability;
mod flow;
mod function;
mod host;
mod layer;
mod metric;
mod predicate;
mod proof;
mod resource;
mod style;
mod trait_impl;
mod use_declaration;
mod view;
mod view_source;

pub(super) fn retained_style_expression_owners(
    items: &ArenaSnapshot<HirItem, ItemId>,
    slots: &SlotSnapshot,
) -> Option<BTreeSet<ExprId>> {
    style::retained_expression_owners(items, slots)
}

impl HirSourceIndex {
    /// Re-derives every accepted item payload without adding an item component
    /// map or a declaration-member query authority.
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive item projection validates every final declaration family against its exact attached owner"
    )]
    pub(crate) fn validates_attached_items(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        items: &ArenaSnapshot<HirItem, ItemId>,
        declaration_members: &HirDeclarationMemberIndex,
        arenas: &ItemValidationArenas<'_>,
    ) -> bool {
        let Ok(attached_items) = parsed.items() else {
            return false;
        };
        let attached_items = attached_items
            .into_iter()
            .map(|item| (item.id(), item))
            .collect::<BTreeMap<_, _>>();
        let Ok(entries) = items.try_iter_prepared(slots) else {
            return false;
        };

        entries.into_iter().all(|(owner, item)| {
            if self
                .syntax_owners
                .contains_key(&SyntheticOwner::Item(owner))
            {
                return false;
            }
            let Ok(metadata) = slots.resolve_prepared(owner) else {
                return false;
            };
            let HirOrigin::Source(source) = metadata.origin() else {
                return false;
            };
            let Some(attached) = attached_items.get(&source.syntax()) else {
                return false;
            };
            if metadata.source_site() != &HirSourceSite::Span(attached.source_span()) {
                return false;
            }
            if !declaration::exact_manifest(self, parsed, owner, attached, item.kind())
                || !entry_source::exact_manifest(self, parsed, owner, attached, item.kind())
                || !callable_source::exact_manifest(self, parsed, owner, attached, item.kind())
                || !use_declaration::exact_manifest(self, parsed, owner, attached, item.kind())
                || !view_source::exact_manifest(self, parsed, owner, attached, item.kind())
            {
                return false;
            }

            match (attached, item.kind()) {
                (TypedItemNode::Module(_), HirItemKind::Module(_)) => {
                    item.members().is_empty() && declaration_members.arena(owner).is_none()
                }
                (TypedItemNode::Use(attached), HirItemKind::Use(retained)) => {
                    use_declaration::payload_matches(attached, retained)
                        && item.members().is_empty()
                        && declaration_members.arena(owner).is_none()
                }
                (TypedItemNode::Error(_), HirItemKind::Error(_)) => {
                    item.prefix().documentation().is_none()
                        && item.prefix().attributes().is_empty()
                        && item.prefix().visibility().is_none()
                        && item.members().is_empty()
                        && item.state()
                            == &HirItemPoisonState::Poisoned(HirItemIssue::UnclassifiedSyntax)
                        && declaration_members.arena(owner).is_none()
                }
                (TypedItemNode::Character(character), HirItemKind::Character(_)) => {
                    character.semantics().is_ok_and(|attached| {
                        character_payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            slots,
                        )
                    })
                }
                (TypedItemNode::Flow(flow), HirItemKind::Flow(_)) => {
                    flow.semantics().is_ok_and(|attached| {
                        flow::payload_matches(self, owner, &attached, item, parsed, slots, arenas)
                            && declaration_members.arena(owner).is_none()
                    })
                }
                (TypedItemNode::Signal(signal), HirItemKind::Signal(_)) => {
                    signal.semantics().is_ok_and(|attached| {
                        signal_payload_matches(&attached, item, slots)
                            && declaration_members.arena(owner).is_none()
                    })
                }
                (TypedItemNode::Action(action), HirItemKind::Action(_)) => {
                    action.semantics().is_ok_and(|attached| {
                        action_payload_matches(owner, &attached, item, slots, arenas)
                            && declaration_members.arena(owner).is_none()
                    })
                }
                (TypedItemNode::Activity(activity), HirItemKind::Activity(_)) => {
                    activity.semantics().is_ok_and(|attached| {
                        activity::payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::Metric(metric), HirItemKind::Metric(_)) => {
                    metric.semantics().is_ok_and(|attached| {
                        metric::payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::Layer(layer), HirItemKind::Layer(_)) => {
                    layer.semantics().is_ok_and(|attached| {
                        layer::payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::View(view), HirItemKind::View(_)) => {
                    view.semantics().is_ok_and(|attached| {
                        view::payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::Function(function), HirItemKind::Function(_)) => {
                    function.semantics().is_ok_and(|attached| {
                        function::payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            parsed,
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::Predicate(predicate), HirItemKind::Predicate(_)) => {
                    predicate.semantics().is_ok_and(|attached| {
                        predicate::payload_matches(
                            self,
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            parsed,
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::Proof(proof), HirItemKind::Proof(_)) => {
                    proof.semantics().is_ok_and(|attached| {
                        proof::payload_matches(
                            self,
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            parsed,
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::Trait(declaration), HirItemKind::Trait(_)) => {
                    declaration.semantics().is_ok_and(|attached| {
                        trait_impl::trait_payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            parsed,
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::Impl(declaration), HirItemKind::Impl(_)) => {
                    declaration.semantics().is_ok_and(|attached| {
                        trait_impl::impl_payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            parsed,
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::ExternCapability(capability), HirItemKind::ExternCapability(_)) => {
                    capability.semantics().is_ok_and(|attached| {
                        extern_capability::payload_matches(
                            owner,
                            &attached,
                            item,
                            declaration_members.arena(owner),
                            slots,
                            arenas,
                        )
                    })
                }
                (TypedItemNode::Test(test), HirItemKind::Test(_)) => {
                    test.semantics().is_ok_and(|attached| {
                        host::test_payload_matches(owner, &attached, item, parsed, slots, arenas)
                            && declaration_members.arena(owner).is_none()
                    })
                }
                (TypedItemNode::Bench(bench), HirItemKind::Bench(_)) => {
                    bench.semantics().is_ok_and(|attached| {
                        host::bench_payload_matches(owner, &attached, item, parsed, slots, arenas)
                            && declaration_members.arena(owner).is_none()
                    })
                }
                (TypedItemNode::Entry(entry), HirItemKind::Entry(_)) => {
                    entry.semantics().is_ok_and(|attached| {
                        entry::payload_matches(owner, &attached, item, parsed, slots, arenas)
                            && declaration_members.arena(owner).is_none()
                    })
                }
                (TypedItemNode::TypeAlias(alias), HirItemKind::TypeAlias(_)) => alias
                    .semantics()
                    .map(AttachedNominalDeclaration::TypeAlias)
                    .is_ok_and(|attached| {
                        nominal_payload_matches(&attached, item, slots)
                            && declaration_members.arena(owner).is_none()
                    }),
                (TypedItemNode::Struct(record), HirItemKind::Struct(_)) => record
                    .semantics()
                    .map(AttachedNominalDeclaration::Struct)
                    .is_ok_and(|attached| {
                        nominal_payload_matches(&attached, item, slots)
                            && declaration_members.arena(owner).is_none()
                    }),
                (TypedItemNode::Enum(choice), HirItemKind::Enum(_)) => choice
                    .semantics()
                    .map(AttachedNominalDeclaration::Enum)
                    .is_ok_and(|attached| {
                        nominal_payload_matches(&attached, item, slots)
                            && declaration_members.arena(owner).is_none()
                    }),
                (TypedItemNode::Resource(resource), HirItemKind::Resource(_)) => {
                    resource.semantics().is_ok_and(|attached| {
                        resource::payload_matches(&attached, item, slots, arenas)
                            && declaration_members.arena(owner).is_none()
                    })
                }
                (TypedItemNode::Style(style_item), HirItemKind::Style(_)) => {
                    style_item.semantics().is_ok_and(|attached| {
                        style::payload_matches(self, owner, &attached, item, parsed, slots)
                            && declaration_members.arena(owner).is_none()
                    })
                }
                _ => false,
            }
        })
    }
}

fn nominal_payload_matches(
    attached: &AttachedNominalDeclaration,
    item: &HirItem,
    slots: &SlotSnapshot,
) -> bool {
    let payload_matches = match (attached, item.kind()) {
        (AttachedNominalDeclaration::TypeAlias(attached), HirItemKind::TypeAlias(retained)) => {
            type_alias_matches(retained, attached, item, slots)
        }
        (AttachedNominalDeclaration::Struct(attached), HirItemKind::Struct(retained)) => {
            struct_matches(retained, attached, item, slots)
        }
        (AttachedNominalDeclaration::Enum(attached), HirItemKind::Enum(retained)) => {
            enum_matches(retained, attached, item, slots)
        }
        _ => false,
    };
    payload_matches && item.members().is_empty()
}

fn type_alias_matches(
    retained: &HirTypeAliasItem,
    attached: &AttachedTypeAliasDeclaration,
    item: &HirItem,
    slots: &SlotSnapshot,
) -> bool {
    item_prefix_matches(item, attached.prefix(), slots)
        && required_name_matches(retained.name(), attached.name())
        && generic_parameters_match(retained.generic_parameters(), attached.generics(), slots)
        && where_predicates_match(retained.where_predicates(), attached.where_clauses(), slots)
        && type_owner_matches(retained.target(), attached.target(), slots)
        && item.state() == &expected_type_alias_state(attached, retained, item.prefix(), slots)
}

fn struct_matches(
    retained: &HirStructItem,
    attached: &arcweft_lang_syntax::attachment::AttachedStructDeclaration,
    item: &HirItem,
    slots: &SlotSnapshot,
) -> bool {
    item_prefix_matches(item, attached.prefix(), slots)
        && required_name_matches(retained.name(), attached.name())
        && generic_parameters_match(retained.generic_parameters(), attached.generics(), slots)
        && where_predicates_match(retained.where_predicates(), attached.where_clauses(), slots)
        && retained.fields().len() == attached.body().fields().len()
        && retained
            .fields()
            .iter()
            .zip(attached.body().fields())
            .all(|(retained, attached)| {
                documentation_matches(retained.documentation(), attached.prefix().documentation())
                    && required_name_matches(retained.name(), attached.name())
                    && type_owner_matches(retained.ty(), attached.ty(), slots)
            })
        && item.state() == &expected_struct_state(attached, retained, item.prefix(), slots)
}

fn enum_matches(
    retained: &HirEnumItem,
    attached: &arcweft_lang_syntax::attachment::AttachedEnumDeclaration,
    item: &HirItem,
    slots: &SlotSnapshot,
) -> bool {
    item_prefix_matches(item, attached.prefix(), slots)
        && required_name_matches(retained.name(), attached.name())
        && generic_parameters_match(retained.generic_parameters(), attached.generics(), slots)
        && where_predicates_match(retained.where_predicates(), attached.where_clauses(), slots)
        && retained.variants().len() == attached.body().variants().len()
        && retained
            .variants()
            .iter()
            .zip(attached.body().variants())
            .all(|(retained, attached)| {
                documentation_matches(retained.documentation(), attached.prefix().documentation())
                    && required_name_matches(retained.name(), attached.name())
                    && match (retained.payload(), attached.payload()) {
                        (Some(retained), Some(attached)) => {
                            type_owner_matches(retained, attached, slots)
                        }
                        (None, None) => true,
                        _ => false,
                    }
            })
        && item.state() == &expected_enum_state(attached, retained, item.prefix(), slots)
}

pub(super) fn item_prefix_matches(
    item: &HirItem,
    attached: &AttachedItemPrefix,
    slots: &SlotSnapshot,
) -> bool {
    prefix_matches(item.prefix(), attached, slots)
}

fn prefix_matches(
    retained_prefix: &crate::item::HirItemPrefix,
    attached: &AttachedItemPrefix,
    slots: &SlotSnapshot,
) -> bool {
    documentation_matches(retained_prefix.documentation(), attached.documentation())
        && retained_attribute_projections(attached, slots).is_ok_and(|attributes| {
            attributes.len() == retained_prefix.attributes().len()
                && attributes.iter().zip(retained_prefix.attributes()).all(
                    |(projected, retained)| {
                        resolved_path_projection_matches(retained.path(), projected.attached.path())
                            && retained.arguments() == projected.arguments.as_ref()
                    },
                )
        })
        && match (retained_prefix.visibility(), attached.visibility()) {
            (None, None) => true,
            (None, Some(attached)) => matches!(
                attached.kind(),
                arcweft_lang_syntax::attachment::source_file::AttachedVisibilityKind::Recovery
            ),
            (Some(crate::item::HirVisibility::Public), Some(attached)) => matches!(
                attached.kind(),
                arcweft_lang_syntax::attachment::source_file::AttachedVisibilityKind::Public
            ),
            (Some(crate::item::HirVisibility::Crate), Some(attached)) => matches!(
                attached.kind(),
                arcweft_lang_syntax::attachment::source_file::AttachedVisibilityKind::Crate
            ),
            (Some(crate::item::HirVisibility::Super), Some(attached)) => matches!(
                attached.kind(),
                arcweft_lang_syntax::attachment::source_file::AttachedVisibilityKind::Super
            ),
            _ => false,
        }
}

fn retained_attribute_projections<'a>(
    prefix: &'a AttachedItemPrefix,
    slots: &SlotSnapshot,
) -> Result<Vec<RetainedAttributeProjection<'a>>, ()> {
    prefix
        .attributes()
        .iter()
        .map(|attribute| retained_attribute_projection(attribute, slots))
        .collect::<Result<Vec<_>, _>>()
        .map(|attributes| attributes.into_iter().flatten().collect())
}

fn retained_attribute_projection<'a>(
    attached: &'a AttachedOuterAttribute,
    slots: &SlotSnapshot,
) -> Result<Option<RetainedAttributeProjection<'a>>, ()> {
    let structurally_recovered = attached.issue().is_some()
        || attached.recovery().is_some()
        || !attached_path_is_resolved(attached.path())
        || attached.form().terminator() == Some(SyntaxCallArgumentListTerminator::RecoveredMissing)
        || matches!(
            attached.close_state(),
            arcweft_lang_syntax::attachment::source_file::AttachedDelimiterState::Missing(_)
        )
        || attached
            .arguments()
            .iter()
            .any(|argument| argument.projection().has_recovery());
    if structurally_recovered {
        return Ok(None);
    }

    let mut arguments = Vec::with_capacity(attached.arguments().len());
    let mut child_states = Vec::with_capacity(attached.arguments().len());
    for attached_argument in attached.arguments() {
        let AttachedAttributeValue::Authored(expression) = attached_argument.value() else {
            return Ok(None);
        };
        let value = slots
            .prepared_source_owner::<ExprId>(expression.id())
            .ok_or(())?;
        let child_state = if slot_is_poisoned(slots, value) {
            HirCallChildPoison::Poisoned
        } else {
            HirCallChildPoison::Clean
        };
        let value = HirCallValue::Present { value };
        let argument = match attached_argument.projection() {
            SyntaxCallArgumentProjection::Positional { .. } => {
                HirCallArgument::Positional { value }
            }
            SyntaxCallArgumentProjection::Named { name, equals, .. } => {
                let Ok(name) = name else {
                    return Ok(None);
                };
                HirCallArgument::Named {
                    name: HirRecoveredName::Valid(
                        HirName::try_new(name.as_str().into()).map_err(|_| ())?,
                    ),
                    equals: attribute_token_state(*equals),
                    value,
                }
            }
            SyntaxCallArgumentProjection::Spread { ellipsis, .. } => HirCallArgument::Spread {
                value,
                ellipsis: attribute_token_state(*ellipsis),
            },
        };
        arguments.push(argument);
        child_states.push(child_state);
    }

    if !HirCallExpr::argument_issues(&arguments, &child_states)
        .map_err(|_| ())?
        .is_empty()
    {
        return Ok(None);
    }
    Ok(Some(RetainedAttributeProjection {
        attached,
        arguments: arguments.into_boxed_slice(),
    }))
}

const fn attribute_token_state(state: SyntaxRequiredTokenState) -> HirRequiredTokenState {
    match state {
        SyntaxRequiredTokenState::Present => HirRequiredTokenState::Present,
        SyntaxRequiredTokenState::Missing => HirRequiredTokenState::Missing,
        SyntaxRequiredTokenState::InvalidPresent => HirRequiredTokenState::InvalidPresent,
    }
}

fn documentation_matches(
    retained: Option<&crate::item::HirDocumentation>,
    attached: Option<&arcweft_lang_syntax::attachment::AttachedDocumentation>,
) -> bool {
    match (retained, attached) {
        (Some(retained), Some(attached)) => retained.markdown() == attached.markdown(),
        (None, None) => true,
        _ => false,
    }
}

fn required_name_matches(retained: &HirRequiredName, attached: &AttachedRequiredName) -> bool {
    match (retained, attached) {
        (HirRequiredName::Resolved(retained), AttachedRequiredName::Resolved { value, .. }) => {
            retained.as_str() == value.as_str()
        }
        (HirRequiredName::Missing, AttachedRequiredName::Missing { .. }) => true,
        _ => false,
    }
}

pub(super) fn generic_parameters_match(
    retained: &[HirGenericParameter],
    attached: Option<&arcweft_lang_syntax::attachment::AttachedGenericParameterGroup>,
    slots: &SlotSnapshot,
) -> bool {
    let attached = attached.map_or(&[][..], |group| group.parameters());
    retained.len() == attached.len()
        && retained
            .iter()
            .zip(attached)
            .all(|(retained, attached)| match (retained, attached) {
                (
                    HirGenericParameter::Lifetime { name: retained },
                    AttachedGenericParameter::Lifetime { name: attached, .. },
                ) => required_name_matches(retained, attached),
                (
                    HirGenericParameter::Type {
                        name: retained_name,
                        bounds: retained_bounds,
                    },
                    AttachedGenericParameter::Type {
                        name: attached_name,
                        bounds: attached_bounds,
                        ..
                    },
                ) => {
                    required_name_matches(retained_name, attached_name)
                        && type_owners_match(retained_bounds, attached_bounds, slots)
                }
                _ => false,
            })
}

pub(super) fn where_predicates_match(
    retained: &[HirWherePredicate],
    clauses: &[AttachedWhereClause],
    slots: &SlotSnapshot,
) -> bool {
    let attached = clauses
        .iter()
        .flat_map(AttachedWhereClause::predicates)
        .collect::<Vec<_>>();
    retained.len() == attached.len()
        && retained.iter().zip(attached).all(|(retained, attached)| {
            type_owner_matches(retained.subject(), attached.subject(), slots)
                && type_owners_match(retained.bounds(), attached.bounds(), slots)
        })
}

fn type_owners_match(
    retained: &[TypeId],
    attached: &[AttachedTypeRefNode],
    slots: &SlotSnapshot,
) -> bool {
    retained.len() == attached.len()
        && retained
            .iter()
            .copied()
            .zip(attached)
            .all(|(retained, attached)| type_owner_matches(retained, attached, slots))
}

pub(super) fn type_owner_matches(
    retained: TypeId,
    attached: &AttachedTypeRefNode,
    slots: &SlotSnapshot,
) -> bool {
    source_matches(slots, retained, attached.id())
}

fn type_tree_is_unallocated(attached: &AttachedTypeRefNode, slots: &SlotSnapshot) -> bool {
    slots
        .prepared_source_owner::<TypeId>(attached.id())
        .is_none()
        && attached.children().is_ok_and(|children| {
            children
                .iter()
                .all(|child| type_tree_is_unallocated(child.node(), slots))
        })
}

pub(super) fn expression_owner_matches(
    retained: ExprId,
    attached: &AttachedExpressionNode,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    source_matches(slots, retained, attached.id())
        && arenas
            .expressions
            .resolve_prepared(slots, retained)
            .is_ok_and(|expression| expression.scope() == scope)
}

fn expected_type_alias_state(
    attached: &AttachedTypeAliasDeclaration,
    retained: &HirTypeAliasItem,
    prefix: &crate::item::HirItemPrefix,
    slots: &SlotSnapshot,
) -> HirItemPoisonState {
    item_state(
        prefix_issue(attached.prefix(), prefix, slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| generic_issue(attached.generics(), retained.generic_parameters(), slots))
            .or_else(|| {
                attached
                    .assignment()
                    .is_missing()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                type_is_poisoned(retained.target(), slots).then_some(HirItemIssue::MissingType)
            })
            .or_else(|| where_issue(attached.where_clauses(), retained.where_predicates(), slots)),
    )
}

fn expected_struct_state(
    attached: &arcweft_lang_syntax::attachment::AttachedStructDeclaration,
    retained: &HirStructItem,
    prefix: &crate::item::HirItemPrefix,
    slots: &SlotSnapshot,
) -> HirItemPoisonState {
    let member_issue = attached
        .body()
        .fields()
        .iter()
        .zip(retained.fields())
        .any(|(attached, retained)| {
            !attached.prefix().attributes().is_empty()
                || attached.name().is_missing()
                || attached.colon().is_missing()
                || type_is_poisoned(retained.ty(), slots)
        })
        .then_some(HirItemIssue::InvalidMember);
    item_state(
        prefix_issue(attached.prefix(), prefix, slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| generic_issue(attached.generics(), retained.generic_parameters(), slots))
            .or_else(|| where_issue(attached.where_clauses(), retained.where_predicates(), slots))
            .or_else(|| {
                matches!(attached.body(), AttachedStructBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(member_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_missing_or_unclosed()
                    .then_some(HirItemIssue::Recovery)
            }),
    )
}

fn expected_enum_state(
    attached: &arcweft_lang_syntax::attachment::AttachedEnumDeclaration,
    retained: &HirEnumItem,
    prefix: &crate::item::HirItemPrefix,
    slots: &SlotSnapshot,
) -> HirItemPoisonState {
    let member_issue = attached
        .body()
        .variants()
        .iter()
        .zip(retained.variants())
        .any(|(attached, retained)| {
            !attached.prefix().attributes().is_empty()
                || attached.name().is_missing()
                || retained
                    .payload()
                    .is_some_and(|payload| type_is_poisoned(payload, slots))
        })
        .then_some(HirItemIssue::InvalidMember);
    item_state(
        prefix_issue(attached.prefix(), prefix, slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| generic_issue(attached.generics(), retained.generic_parameters(), slots))
            .or_else(|| where_issue(attached.where_clauses(), retained.where_predicates(), slots))
            .or_else(|| {
                matches!(attached.body(), AttachedEnumBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(member_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_missing_or_unclosed()
                    .then_some(HirItemIssue::Recovery)
            }),
    )
}

fn signal_payload_matches(
    attached: &AttachedSignalDeclaration,
    item: &HirItem,
    slots: &SlotSnapshot,
) -> bool {
    let HirItemKind::Signal(signal) = item.kind() else {
        return false;
    };
    item_prefix_matches(item, attached.prefix(), slots)
        && retained_header_matches(signal.header(), attached.header())
        && type_owner_matches(signal.observable_type(), attached.observable_type(), slots)
        && item.members().is_empty()
        && item.state()
            == &signal_item_state(attached, signal.observable_type(), item.prefix(), slots)
}

fn signal_item_state(
    attached: &AttachedSignalDeclaration,
    observable_type: TypeId,
    prefix: &crate::item::HirItemPrefix,
    slots: &SlotSnapshot,
) -> HirItemPoisonState {
    let type_issue = type_is_poisoned(observable_type, slots).then_some(
        if attached.observable_type().syntax().kind() == SyntaxKind::MissingType {
            HirItemIssue::MissingType
        } else {
            HirItemIssue::Recovery
        },
    );
    item_state(
        prefix_issue(attached.prefix(), prefix, slots)
            .or_else(|| retained_header_item_issue(attached.header()))
            .or_else(|| {
                attached
                    .colon()
                    .is_missing()
                    .then_some(HirItemIssue::Recovery)
            })
            .or(type_issue)
            .or_else(|| {
                attached
                    .trailing_recovery()
                    .is_some()
                    .then_some(HirItemIssue::Recovery)
            }),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one action-family matrix proves every typed payload and recovery shape without a parallel reader"
)]
fn action_payload_matches(
    owner: ItemId,
    attached: &AttachedActionDeclaration,
    item: &HirItem,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Action(action) = item.kind() else {
        return false;
    };
    let callable_scope = action.callable_scope();
    let Ok(scope) = arenas.scopes.resolve_prepared(slots, callable_scope) else {
        return false;
    };
    let Ok(root_scope) = arenas.scopes.resolve_prepared(slots, item.scope()) else {
        return false;
    };
    let Ok(local_entries) = arenas.locals.try_iter_prepared(slots) else {
        return false;
    };
    let local_entries = local_entries.collect::<Vec<_>>();
    let scope_site_matches = slots
        .resolve_prepared(callable_scope)
        .is_ok_and(|metadata| {
            metadata.source_site() == &HirSourceSite::Span(attached.syntax().source_span())
        });
    if !item_prefix_matches(item, attached.prefix(), slots)
        || !retained_header_matches(action.header(), attached.header())
        || !item.members().is_empty()
        || !source_matches(slots, callable_scope, attached.syntax().id())
        || !scope_site_matches
        || scope.kind() != HirScopeKind::Callable
        || scope.parent() != Some(item.scope())
        || scope.owner() != &HirScopeOwner::Item(owner)
        || !scope.children().is_empty()
        || !root_scope.children().contains(&callable_scope)
        || action.parameters().len() != attached.signature().parameters().len()
    {
        return false;
    }

    let mut all_pattern_owners = BTreeSet::new();
    let mut flattened_locals = Vec::new();
    let mut generations = BTreeMap::new();
    let binding_arenas = BlockValidationArenas {
        expressions: arenas.expressions,
        statements: arenas.statements,
        scopes: arenas.scopes,
        locals: arenas.locals,
        patterns: arenas.patterns,
    };
    for (position, (retained, source)) in action
        .parameters()
        .iter()
        .zip(attached.signature().parameters())
        .enumerate()
    {
        if usize::from(source.source_ordinal()) != position
            || retained.default().is_some()
            || !source_matches(slots, retained.pattern(), source.pattern().id())
            || !source_matches(slots, retained.ty(), source.ty().id())
            || source.forbidden_default().is_some_and(|default| {
                !expression_tree_is_unallocated(default.value(), slots, &mut BTreeSet::new())
            })
        {
            return false;
        }
        let Ok(pattern) = arenas.patterns.resolve_prepared(slots, retained.pattern()) else {
            return false;
        };
        let Ok(ty) = arenas.types.resolve_prepared(slots, retained.ty()) else {
            return false;
        };
        if pattern.scope() != callable_scope || ty.scope() != callable_scope {
            return false;
        }
        let Some(expected_locals) = canonical_pattern_locals(
            slots,
            &binding_arenas,
            retained.pattern(),
            retained.pattern(),
            callable_scope,
        ) else {
            return false;
        };
        let expected_local_ids = expected_locals
            .iter()
            .map(|expected| expected.local)
            .collect::<Vec<_>>();
        let mut local_validation = BindingLocalValidation::new(
            callable_scope,
            HirPatternBindingPolicy::CallableParameter,
            &mut generations,
            slots,
            arenas.patterns,
            arenas.locals,
        );
        if expected_local_ids.as_slice() != retained.locals()
            || !binding_locals_match(source.pattern(), &expected_locals, &mut local_validation)
        {
            return false;
        }

        let mut pattern_owners = BTreeSet::new();
        if !collect_pattern_subtree(
            retained.pattern(),
            slots,
            arenas.patterns,
            &mut pattern_owners,
        ) || pattern_owners
            .iter()
            .any(|pattern| !all_pattern_owners.insert(*pattern))
        {
            return false;
        }
        let owned_locals = local_entries
            .iter()
            .filter_map(|(local, payload)| {
                (payload.scope() == callable_scope
                    && payload
                        .pattern()
                        .is_some_and(|pattern| pattern_owners.contains(&pattern)))
                .then_some((*local, *payload))
            })
            .collect::<Vec<_>>();
        let expected_local_set = owned_locals
            .iter()
            .map(|(local, _)| *local)
            .collect::<BTreeSet<_>>();
        let retained_local_set = retained.locals().iter().copied().collect::<BTreeSet<_>>();
        if retained_local_set.len() != retained.locals().len()
            || retained_local_set != expected_local_set
            || !locals_follow_source_order(retained.locals(), slots)
            || owned_locals.iter().any(|(_, local)| {
                local.kind() != HirLocalKind::Parameter
                    || local
                        .pattern()
                        .is_none_or(|pattern| !pattern_owners.contains(&pattern))
            })
        {
            return false;
        }
        flattened_locals.extend_from_slice(retained.locals());
    }

    let flattened_set = flattened_locals.iter().copied().collect::<BTreeSet<_>>();
    let all_scope_locals = local_entries
        .iter()
        .filter_map(|(local, payload)| (payload.scope() == callable_scope).then_some(*local))
        .collect::<BTreeSet<_>>();
    scope.locals() == flattened_locals.as_slice()
        && flattened_set.len() == flattened_locals.len()
        && flattened_set == all_scope_locals
        && item.state() == &action_item_state(attached, action, item.prefix(), slots)
}

fn collect_pattern_subtree(
    owner: PatternId,
    slots: &SlotSnapshot,
    patterns: &ArenaSnapshot<HirPattern, PatternId>,
    retained: &mut BTreeSet<PatternId>,
) -> bool {
    if !retained.insert(owner) {
        return false;
    }
    let Ok(pattern) = patterns.resolve_prepared(slots, owner) else {
        return false;
    };
    super::pattern_projection::pattern_child_ids(pattern.kind())
        .into_iter()
        .all(|child| collect_pattern_subtree(child, slots, patterns, retained))
}

fn expression_tree_is_unallocated(
    attached: &AttachedExpressionNode,
    slots: &SlotSnapshot,
    visited: &mut BTreeSet<arcweft_lang_syntax::attachment::SyntaxNodeId>,
) -> bool {
    if !visited.insert(attached.id())
        || slots
            .prepared_source_owner::<ExprId>(attached.id())
            .is_some()
    {
        return false;
    }
    attached.children().iter().all(|child| {
        child.authored_semantic().is_ok_and(|child| {
            child.is_none_or(|child| expression_tree_is_unallocated(&child, slots, visited))
        })
    })
}

fn locals_follow_source_order(locals: &[LocalId], slots: &SlotSnapshot) -> bool {
    let mut previous = None;
    for &local in locals {
        let Ok(metadata) = slots.resolve_prepared(local) else {
            return false;
        };
        let start = match metadata.source_site() {
            HirSourceSite::Span(span) => span.range().start(),
            HirSourceSite::Insertion(insertion) => insertion.offset(),
        };
        if previous.is_some_and(|previous| previous > start) {
            return false;
        }
        previous = Some(start);
    }
    true
}

fn action_item_state(
    attached: &AttachedActionDeclaration,
    retained: &HirActionDeclaration,
    prefix: &crate::item::HirItemPrefix,
    slots: &SlotSnapshot,
) -> HirItemPoisonState {
    let parameter_issue = attached
        .signature()
        .parameters()
        .iter()
        .zip(retained.parameters())
        .find_map(|(attached, retained)| {
            let type_poisoned = type_is_poisoned(retained.ty(), slots);
            attached
                .has_invalid_binding()
                .then_some(HirItemIssue::InvalidMember)
                .or_else(|| {
                    (attached.colon().is_missing()
                        || (type_poisoned
                            && attached.ty().syntax().kind() == SyntaxKind::MissingType))
                        .then_some(HirItemIssue::MissingType)
                })
                .or_else(|| type_poisoned.then_some(HirItemIssue::InvalidMember))
                .or_else(|| {
                    attached
                        .forbidden_default()
                        .is_some()
                        .then_some(HirItemIssue::InvalidMember)
                })
        });
    item_state(
        prefix_issue(attached.prefix(), prefix, slots)
            .or_else(|| retained_header_item_issue(attached.header()))
            .or_else(|| {
                (attached.signature().open_state().is_missing()
                    || attached.signature().close_state().is_missing())
                .then_some(HirItemIssue::Recovery)
            })
            .or(parameter_issue)
            .or_else(|| {
                attached
                    .trailing_recovery()
                    .is_some()
                    .then_some(HirItemIssue::Recovery)
            }),
    )
}

fn prefix_issue(
    attached: &AttachedItemPrefix,
    retained: &crate::item::HirItemPrefix,
    slots: &SlotSnapshot,
) -> Option<HirItemIssue> {
    retained_attribute_projections(attached, slots)
        .ok()
        .and_then(|projected| {
            (projected.len() != attached.attributes().len()
                || projected.len() != retained.attributes().len())
            .then_some(HirItemIssue::Recovery)
        })
        .or_else(|| {
            attached
                .visibility()
                .is_some_and(|visibility| {
                    matches!(
                        visibility.kind(),
                        arcweft_lang_syntax::attachment::source_file::AttachedVisibilityKind::Recovery
                    )
                })
                .then_some(HirItemIssue::MalformedHeader)
        })
}

fn name_issue(name: &AttachedRequiredName) -> Option<HirItemIssue> {
    name.is_missing().then_some(HirItemIssue::MissingName)
}

fn generic_issue(
    attached: Option<&arcweft_lang_syntax::attachment::AttachedGenericParameterGroup>,
    retained: &[HirGenericParameter],
    slots: &SlotSnapshot,
) -> Option<HirItemIssue> {
    attached.and_then(|attached| {
        (attached.has_recovery()
            || retained.iter().any(|parameter| {
                parameter
                    .bounds()
                    .iter()
                    .copied()
                    .any(|bound| type_is_poisoned(bound, slots))
            }))
        .then_some(HirItemIssue::Recovery)
    })
}

fn where_issue(
    attached: &[AttachedWhereClause],
    retained: &[HirWherePredicate],
    slots: &SlotSnapshot,
) -> Option<HirItemIssue> {
    attached
        .iter()
        .flat_map(AttachedWhereClause::predicates)
        .zip(retained)
        .any(|(attached, retained)| {
            attached.has_recovery()
                || type_is_poisoned(retained.subject(), slots)
                || retained
                    .bounds()
                    .iter()
                    .copied()
                    .any(|bound| type_is_poisoned(bound, slots))
        })
        .then_some(HirItemIssue::Recovery)
}

fn type_is_poisoned(owner: TypeId, slots: &SlotSnapshot) -> bool {
    slot_is_poisoned(slots, owner)
}

const fn item_state(issue: Option<HirItemIssue>) -> HirItemPoisonState {
    match issue {
        Some(issue) => HirItemPoisonState::Poisoned(issue),
        None => HirItemPoisonState::Clean,
    }
}

fn retained_header_matches(
    retained: &crate::item::HirRetainedHeader,
    attached: &AttachedRetainedHeader,
) -> bool {
    let name_matches = match (retained.name(), attached.name()) {
        (HirRetainedName::Resolved(retained), AttachedRetainedName::Resolved { value, .. }) => {
            retained.as_str() == value.as_str()
        }
        (HirRetainedName::Missing, AttachedRetainedName::Missing { .. })
        | (HirRetainedName::Invalid, AttachedRetainedName::Invalid { .. }) => true,
        _ => false,
    };
    name_matches
        && match (retained.public_id(), attached.public_id()) {
            (
                HirRetainedPublicId::Resolved {
                    origin: HirPublicIdOrigin::DerivedFromName,
                    ..
                },
                AttachedDeclarationPublicId::Derived,
            ) if matches!(attached.name(), AttachedRetainedName::Resolved { .. }) => true,
            (
                HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::DerivedFromRecoveredName),
                AttachedDeclarationPublicId::Derived,
            ) if !matches!(attached.name(), AttachedRetainedName::Resolved { .. }) => true,
            (
                HirRetainedPublicId::Resolved {
                    value: retained,
                    origin: HirPublicIdOrigin::Explicit,
                },
                AttachedDeclarationPublicId::Explicit { value, .. },
            ) => retained == value,
            (
                HirRetainedPublicId::Recovered(retained),
                AttachedDeclarationPublicId::Recovered { issue, .. },
            ) => retained_public_id_issue_matches(retained, issue),
            _ => false,
        }
}

fn retained_public_id_issue_matches(
    retained: &HirRetainedPublicIdIssue,
    attached: &AttachedDeclarationPublicIdIssue,
) -> bool {
    match (retained, attached) {
        (HirRetainedPublicIdIssue::Malformed, AttachedDeclarationPublicIdIssue::Malformed)
        | (HirRetainedPublicIdIssue::Missing, AttachedDeclarationPublicIdIssue::Missing) => true,
        (
            HirRetainedPublicIdIssue::WrongFamily(retained),
            AttachedDeclarationPublicIdIssue::WrongFamily(attached),
        ) => retained == attached,
        _ => false,
    }
}

fn character_payload_matches(
    owner: ItemId,
    attached: &AttachedCharacterDeclaration,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    slots: &SlotSnapshot,
) -> bool {
    let HirItemKind::Character(character) = item.kind() else {
        return false;
    };
    item_prefix_matches(item, attached.prefix(), slots)
        && retained_header_matches(character.header(), attached.header())
        && surface_alias_matches(character.surface_alias(), attached.surface_alias())
        && character_members_match(
            owner,
            character.display_name(),
            item,
            members,
            attached,
            slots,
        )
        && item.state() == &character_item_state(attached, members, item.prefix(), slots)
}

fn surface_alias_matches(
    retained: &HirCharacterSurfaceAlias,
    attached: &AttachedCharacterSurfaceAlias,
) -> bool {
    match (retained, attached) {
        (HirCharacterSurfaceAlias::Absent, AttachedCharacterSurfaceAlias::Absent)
        | (HirCharacterSurfaceAlias::Missing, AttachedCharacterSurfaceAlias::Missing { .. }) => {
            true
        }
        (
            HirCharacterSurfaceAlias::Resolved(retained),
            AttachedCharacterSurfaceAlias::Resolved { value, .. },
        ) => retained.as_str() == value.as_str(),
        _ => false,
    }
}

fn character_members_match(
    owner: ItemId,
    display_name: Option<HirDeclarationMemberId>,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    attached: &AttachedCharacterDeclaration,
    slots: &SlotSnapshot,
) -> bool {
    let attached_members = attached.body().members();
    if attached_members.is_empty() {
        return item.members().is_empty() && members.is_none() && display_name.is_none();
    }
    let Some(members) = members else {
        return false;
    };
    if members.owner() != owner
        || members.family() != item.family()
        || members.members().len() != attached_members.len()
        || item.members().len() != attached_members.len()
    {
        return false;
    }
    let mut first_display_name = None;
    for (position, (retained, attached)) in
        members.members().iter().zip(attached_members).enumerate()
    {
        let Ok(ordinal) = u32::try_from(position) else {
            return false;
        };
        let expected = HirDeclarationMemberId::new(owner, ordinal);
        if retained.id() != expected
            || item.members().get(position) != Some(&expected)
            || u32::from(attached.source_ordinal()) != ordinal
        {
            return false;
        }
        match (retained.kind(), attached) {
            (
                HirDeclarationMemberKind::CharacterDisplayName(display),
                AttachedCharacterMember::DisplayName(attached),
            ) => {
                first_display_name.get_or_insert(expected);
                let expected_assignment = if attached.assignment().is_missing() {
                    HirCharacterAssignmentState::Missing
                } else {
                    HirCharacterAssignmentState::Present
                };
                let initializer_matches = match (display.initializer(), attached.initializer()) {
                    (Some(expression), AttachedCharacterInitializer::Authored(attached)) => {
                        source_matches(slots, expression, attached.id())
                    }
                    (None, AttachedCharacterInitializer::Missing(_)) => true,
                    _ => false,
                };
                if display.assignment() != expected_assignment
                    || display.is_duplicate() != attached.is_duplicate()
                    || !initializer_matches
                    || !character_member_state_matches(retained, attached, slots)
                {
                    return false;
                }
            }
            (
                HirDeclarationMemberKind::CharacterRecovery(_),
                AttachedCharacterMember::Recovery { .. },
            ) if retained.state()
                == HirDeclarationMemberPoisonState::Poisoned(
                    HirDeclarationMemberIssue::UnclassifiedSyntax,
                ) => {}
            _ => return false,
        }
    }
    display_name == first_display_name
}

fn character_member_state_matches(
    retained: &HirDeclarationMember,
    attached: &AttachedCharacterDisplayNameMember,
    slots: &SlotSnapshot,
) -> bool {
    if attached.is_duplicate() {
        return retained.state()
            == HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::Duplicate);
    }
    if attached.assignment().is_missing() {
        return retained.state()
            == HirDeclarationMemberPoisonState::Poisoned(
                HirDeclarationMemberIssue::MissingAssignment,
            );
    }
    let HirDeclarationMemberKind::CharacterDisplayName(display) = retained.kind() else {
        return false;
    };
    let Some(initializer) = display.initializer() else {
        return retained.state()
            == HirDeclarationMemberPoisonState::Poisoned(
                HirDeclarationMemberIssue::MissingInitializer,
            );
    };
    let Ok(metadata) = slots.resolve_prepared(initializer) else {
        return false;
    };
    if metadata.is_poisoned() {
        retained.state()
            == HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
    } else {
        retained.state() == HirDeclarationMemberPoisonState::Clean
    }
}

fn character_item_state(
    attached: &AttachedCharacterDeclaration,
    members: Option<&HirDeclarationMemberArena>,
    prefix: &crate::item::HirItemPrefix,
    slots: &SlotSnapshot,
) -> HirItemPoisonState {
    item_state(
        prefix_issue(attached.prefix(), prefix, slots)
            .or_else(|| retained_header_item_issue(attached.header()))
            .or_else(|| {
                attached
                    .has_unexpected_header()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                matches!(
                    attached.surface_alias(),
                    AttachedCharacterSurfaceAlias::Missing { .. }
                )
                .then_some(HirItemIssue::MissingName)
            })
            .or_else(|| {
                matches!(attached.body(), AttachedCharacterBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or_else(|| {
                members
                    .is_some_and(|members| {
                        members
                            .members()
                            .iter()
                            .any(HirDeclarationMember::is_poisoned)
                    })
                    .then_some(HirItemIssue::InvalidMember)
            })
            .or_else(|| {
                (attached.body().is_missing_or_unclosed() || attached.has_trailing_syntax())
                    .then_some(HirItemIssue::Recovery)
            }),
    )
}

fn retained_header_item_issue(attached: &AttachedRetainedHeader) -> Option<HirItemIssue> {
    match attached.name() {
        AttachedRetainedName::Missing { .. } => Some(HirItemIssue::MissingName),
        AttachedRetainedName::Invalid { .. } => Some(HirItemIssue::MalformedHeader),
        AttachedRetainedName::Resolved { .. } => None,
    }
    .or_else(|| {
        matches!(
            attached.public_id(),
            AttachedDeclarationPublicId::Recovered { .. }
        )
        .then_some(HirItemIssue::MalformedHeader)
    })
}

fn source_matches<I: HirTypedId>(
    slots: &SlotSnapshot,
    owner: I,
    syntax: arcweft_lang_syntax::attachment::SyntaxNodeId,
) -> bool {
    slots.resolve_prepared(owner).is_ok_and(
        |metadata| matches!(metadata.origin(), HirOrigin::Source(source) if source.syntax() == syntax),
    )
}

fn slot_is_poisoned<I: HirTypedId>(slots: &SlotSnapshot, owner: I) -> bool {
    slots
        .resolve_prepared(owner)
        .is_ok_and(HirSlotMetadata::is_poisoned)
}
