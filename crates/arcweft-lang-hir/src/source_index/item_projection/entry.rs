//! Entry payload re-derivation for final item publication.

use std::collections::BTreeSet;

use arcweft_lang_syntax::attachment::source_file::AttachedPath;
use arcweft_lang_syntax::attachment::{
    AttachedEntryBody, AttachedEntryDeclaration, AttachedEntryHttpMethod, AttachedEntryId,
    AttachedEntryKind, AttachedEntryMember, AttachedEntryName, AttachedEntryPunctuation,
    AttachedEntryRoleBinding, AttachedEntryRouteBinding, AttachedEntryRouteBindings,
    AttachedEntryValue, AttachedExpressionNode, AttachedTypeRefNode,
};
use arcweft_lang_syntax::expressions::ExpressionProjection;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::identity::{ExprId, ItemId};
use crate::item::{
    HirEntryDeclaration, HirEntryId, HirEntryKind, HirEntryKindIssue, HirEntryMember,
    HirEntryOptionValue, HirEntryPathBinding, HirEntryPathValue, HirEntryPunctuationState,
    HirEntryRoute, HirEntryRouteBinding, HirEntryRouteBindings, HirEntryTarget,
    HirEntryTypeBinding, HirHttpMethod, HirHttpMethodIssue, HirHttpMethodValue, HirItem,
    HirItemIssue, HirItemKind, HirItemPoisonState, HirRequiredName, HirRoutePathIssue,
    HirRoutePathValue,
};
use crate::leaf::{HirLiteral, HirStringLiteral};
use crate::slot::SlotSnapshot;

use super::super::expression_manifest::leaf::path_projection_matches;
use super::{
    ItemValidationArenas, expression_owner_matches, expression_tree_is_unallocated,
    item_prefix_matches, item_state, prefix_issue, slot_is_poisoned, type_owner_matches,
};

pub(super) fn payload_matches(
    _owner: ItemId,
    attached: &AttachedEntryDeclaration,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Entry(retained) = item.kind() else {
        return false;
    };
    let Some(member_issue) = body_matches(retained, attached, item, parsed, slots, arenas) else {
        return false;
    };
    item_prefix_matches(item, attached.prefix(), slots)
        && kind_matches(retained.kind(), attached.kind())
        && id_matches(retained.id(), attached.id())
        && entry_id_is_unallocated(attached.id(), slots)
        && retained.has_header_trailing_recovery() == attached.has_header_trailing_recovery()
        && item.members().is_empty()
        && item.state() == &entry_item_state(attached, retained, item, slots, member_issue)
}

fn body_matches(
    retained: &HirEntryDeclaration,
    attached: &AttachedEntryDeclaration,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    match attached.body() {
        AttachedEntryBody::Missing(_) => {
            matches!(retained.body(), crate::item::HirEntryBody::Missing).then_some(false)
        }
        AttachedEntryBody::Braced { members, .. } => {
            let crate::item::HirEntryBody::Braced {
                members: retained_members,
                closed,
            } = retained.body()
            else {
                return None;
            };
            if *closed != attached.body().is_closed() || retained_members.len() != members.len() {
                return None;
            }
            let mut recovery = false;
            for (retained, attached) in retained_members.iter().zip(members) {
                let member_recovery =
                    member_matches(retained, attached, item, parsed, slots, arenas)?;
                recovery |= member_recovery;
            }
            Some(recovery)
        }
    }
}

fn member_matches(
    retained: &HirEntryMember,
    attached: &AttachedEntryMember,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    match (retained, attached) {
        (HirEntryMember::StateType(retained), AttachedEntryMember::StateType(attached))
        | (HirEntryMember::EventType(retained), AttachedEntryMember::EventType(attached)) => {
            type_binding_matches(retained, attached, item, slots, arenas)
        }
        (HirEntryMember::Initializer(retained), AttachedEntryMember::Initializer(attached))
        | (HirEntryMember::Reducer(retained), AttachedEntryMember::Reducer(attached))
        | (HirEntryMember::Controller(retained), AttachedEntryMember::Controller(attached)) => {
            path_binding_matches(retained, attached, slots)
        }
        (
            HirEntryMember::Goto(retained),
            AttachedEntryMember::Goto {
                target,
                trailing_recovery,
                ..
            },
        ) => {
            let matches = target_matches(retained.target(), target)
                && retained.has_trailing_recovery() == trailing_recovery.is_some()
                && entry_expression_is_unallocated(target, parsed, slots);
            matches.then_some(retained.has_recovery())
        }
        (HirEntryMember::Route(retained), attached @ AttachedEntryMember::Route { .. }) => {
            route_matches(retained, attached, parsed, slots)
        }
        (
            HirEntryMember::Option(retained),
            AttachedEntryMember::Option {
                source_ordinal: _,
                name,
                assignment,
                value,
                trailing_recovery,
                ..
            },
        ) => {
            let child_matches =
                option_expression_matches(retained.value(), value, item, parsed, slots, arenas);
            let child_recovery = retained
                .value()
                .expression()
                .is_some_and(|expression| slot_is_poisoned(slots, expression));
            (name_matches(retained.name(), name)
                && punctuation_matches(retained.assignment(), assignment)
                && retained.has_trailing_recovery() == trailing_recovery.is_some()
                && child_matches)
                .then_some(
                    name.has_recovery()
                        || assignment.is_missing()
                        || value.has_recovery()
                        || child_recovery
                        || trailing_recovery.is_some(),
                )
        }
        (HirEntryMember::Error, AttachedEntryMember::Error { .. }) => Some(true),
        _ => None,
    }
}

fn type_binding_matches(
    retained: &HirEntryTypeBinding,
    attached: &AttachedEntryRoleBinding<AttachedTypeRefNode>,
    item: &HirItem,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    let attached_type = attached.value().value()?;
    let type_record = arenas.types.resolve_prepared(slots, retained.ty()).ok()?;
    (punctuation_matches(retained.assignment(), attached.assignment())
        && retained.has_trailing_recovery() == attached.has_trailing_recovery()
        && type_owner_matches(retained.ty(), attached_type, slots)
        && type_record.scope() == item.scope())
    .then_some(attached.has_recovery() || slot_is_poisoned(slots, retained.ty()))
}

fn path_binding_matches(
    retained: &HirEntryPathBinding,
    attached: &AttachedEntryRoleBinding<AttachedPath>,
    slots: &SlotSnapshot,
) -> Option<bool> {
    let value_matches = match (retained.value(), attached.value()) {
        (
            HirEntryPathValue::Authored(retained),
            AttachedEntryValue::Authored(attached) | AttachedEntryValue::Recovered(attached),
        ) => {
            path_projection_matches(retained, attached)
                && slots
                    .prepared_source_owner::<ExprId>(attached.syntax().id())
                    .is_none()
        }
        (HirEntryPathValue::Missing, AttachedEntryValue::Missing(syntax))
        | (HirEntryPathValue::Invalid, AttachedEntryValue::Invalid(syntax)) => {
            slots.prepared_source_owner::<ExprId>(syntax.id()).is_none()
        }
        _ => false,
    };
    (value_matches
        && punctuation_matches(retained.assignment(), attached.assignment())
        && retained.has_trailing_recovery() == attached.has_trailing_recovery())
    .then_some(retained.has_recovery())
}

fn route_matches(
    retained: &HirEntryRoute,
    attached: &AttachedEntryMember,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
) -> Option<bool> {
    let AttachedEntryMember::Route {
        method,
        path,
        arrow,
        target,
        bindings,
        trailing_recovery,
        ..
    } = attached
    else {
        return None;
    };
    (method_matches(retained.method(), method)
        && route_path_matches(retained.path(), path)
        && punctuation_matches(retained.arrow(), arrow)
        && target_matches(retained.target(), target)
        && route_bindings_match(retained.bindings(), bindings)
        && retained.has_trailing_recovery() == trailing_recovery.is_some()
        && entry_expression_is_unallocated(path, parsed, slots)
        && entry_expression_is_unallocated(target, parsed, slots))
    .then_some(retained.has_recovery())
}

fn kind_matches(retained: &HirEntryKind, attached: &AttachedEntryKind) -> bool {
    match (retained, attached) {
        (HirEntryKind::Game, AttachedEntryKind::Game(_))
        | (HirEntryKind::Editor, AttachedEntryKind::Editor(_))
        | (HirEntryKind::Cli, AttachedEntryKind::Cli(_))
        | (HirEntryKind::Server, AttachedEntryKind::Server(_))
        | (HirEntryKind::Activity, AttachedEntryKind::Activity(_))
        | (HirEntryKind::Test, AttachedEntryKind::Test(_))
        | (HirEntryKind::Bench, AttachedEntryKind::Bench(_))
        | (HirEntryKind::Agent, AttachedEntryKind::Agent(_))
        | (HirEntryKind::Recovered(HirEntryKindIssue::Missing), AttachedEntryKind::Missing(_)) => {
            true
        }
        (HirEntryKind::Custom(retained), AttachedEntryKind::Custom { value, .. }) => {
            retained.as_str() == value.as_str()
        }
        _ => false,
    }
}

fn id_matches(retained: &HirEntryId, attached: &AttachedEntryId) -> bool {
    match (retained, attached) {
        (
            HirEntryId::Authored {
                value: retained,
                canonical_entry_family,
            },
            AttachedEntryId::Authored {
                reference,
                canonical_entry_family: attached_family,
                ..
            },
        ) => {
            canonical_entry_family == attached_family
                && crate::final_lowering::id_ref_projection::id_ref(reference)
                    .is_ok_and(|attached| attached == *retained)
        }
        (HirEntryId::Missing, AttachedEntryId::Missing(_)) => true,
        _ => false,
    }
}

fn entry_id_is_unallocated(attached: &AttachedEntryId, slots: &SlotSnapshot) -> bool {
    match attached {
        AttachedEntryId::Authored { expression, .. } => {
            expression_tree_is_unallocated(expression, slots, &mut BTreeSet::new())
        }
        AttachedEntryId::Missing(syntax) => {
            slots.prepared_source_owner::<ExprId>(syntax.id()).is_none()
        }
    }
}

fn target_matches(
    retained: &HirEntryTarget,
    attached: &AttachedEntryValue<AttachedExpressionNode>,
) -> bool {
    match (retained, attached) {
        (
            HirEntryTarget::Authored(retained),
            AttachedEntryValue::Authored(attached) | AttachedEntryValue::Recovered(attached),
        ) => {
            let ExpressionProjection::EntityReference(reference) = attached.projection() else {
                return false;
            };
            crate::final_lowering::id_ref_projection::id_ref(reference)
                .is_ok_and(|attached| attached == *retained)
        }
        (HirEntryTarget::Missing, AttachedEntryValue::Missing(_))
        | (HirEntryTarget::Invalid, AttachedEntryValue::Invalid(_)) => true,
        _ => false,
    }
}

fn method_matches(retained: &HirHttpMethodValue, attached: &AttachedEntryHttpMethod) -> bool {
    if let HirHttpMethodValue::Resolved(retained) = retained {
        return resolved_http_method(attached).is_some_and(|attached| attached == *retained);
    }
    match (retained, attached) {
        (
            HirHttpMethodValue::Recovered {
                authored: None,
                issue: HirHttpMethodIssue::Missing,
            },
            AttachedEntryHttpMethod::Missing(_),
        )
        | (
            HirHttpMethodValue::Recovered {
                authored: None,
                issue: HirHttpMethodIssue::InvalidName,
            },
            AttachedEntryHttpMethod::Unsupported { value: None, .. },
        ) => true,
        (
            HirHttpMethodValue::Recovered {
                authored: Some(retained),
                issue: HirHttpMethodIssue::Unsupported,
            },
            AttachedEntryHttpMethod::Unsupported {
                value: Some(attached),
                ..
            },
        ) => retained.as_str() == attached.as_str(),
        _ => false,
    }
}

fn resolved_http_method(attached: &AttachedEntryHttpMethod) -> Option<HirHttpMethod> {
    match attached {
        AttachedEntryHttpMethod::Get(_) => Some(HirHttpMethod::Get),
        AttachedEntryHttpMethod::Post(_) => Some(HirHttpMethod::Post),
        AttachedEntryHttpMethod::Put(_) => Some(HirHttpMethod::Put),
        AttachedEntryHttpMethod::Patch(_) => Some(HirHttpMethod::Patch),
        AttachedEntryHttpMethod::Delete(_) => Some(HirHttpMethod::Delete),
        AttachedEntryHttpMethod::Head(_) => Some(HirHttpMethod::Head),
        AttachedEntryHttpMethod::Options(_) => Some(HirHttpMethod::Options),
        AttachedEntryHttpMethod::Unsupported { .. } | AttachedEntryHttpMethod::Missing(_) => None,
    }
}

fn route_path_matches(
    retained: &HirRoutePathValue,
    attached: &AttachedEntryValue<AttachedExpressionNode>,
) -> bool {
    match attached {
        AttachedEntryValue::Authored(expression) | AttachedEntryValue::Recovered(expression) => {
            let ExpressionProjection::Literal(literal) = expression.projection() else {
                return false;
            };
            let Ok(projected) = crate::final_lowering::literal_projection::literal(literal) else {
                return false;
            };
            match (retained, projected) {
                (
                    HirRoutePathValue::Resolved(retained),
                    HirLiteral::String(HirStringLiteral::Value(attached)),
                ) => retained.as_str() == attached.as_ref(),
                (
                    HirRoutePathValue::Recovered {
                        decoded: Some(retained),
                        issue: HirRoutePathIssue::InvalidPath,
                    },
                    HirLiteral::String(HirStringLiteral::Value(attached)),
                ) => retained.as_ref() == attached.as_ref(),
                (
                    HirRoutePathValue::Recovered {
                        decoded: None,
                        issue: HirRoutePathIssue::InvalidString(retained),
                    },
                    HirLiteral::String(HirStringLiteral::Invalid(attached)),
                ) => *retained == attached,
                _ => false,
            }
        }
        AttachedEntryValue::Missing(_) => matches!(
            retained,
            HirRoutePathValue::Recovered {
                decoded: None,
                issue: HirRoutePathIssue::Missing,
            }
        ),
        AttachedEntryValue::Invalid(_) => matches!(
            retained,
            HirRoutePathValue::Recovered {
                decoded: None,
                issue: HirRoutePathIssue::InvalidExpression,
            }
        ),
    }
}

fn route_bindings_match(
    retained: &HirEntryRouteBindings,
    attached: &AttachedEntryRouteBindings,
) -> bool {
    match (retained, attached) {
        (HirEntryRouteBindings::Absent, AttachedEntryRouteBindings::Absent) => true,
        (
            HirEntryRouteBindings::Parenthesized {
                items,
                closed: retained_closed,
            },
            AttachedEntryRouteBindings::Parenthesized { bindings, .. },
        ) => {
            *retained_closed == attached.is_closed()
                && items.len() == bindings.len()
                && items
                    .iter()
                    .zip(bindings)
                    .all(|(retained, attached)| route_binding_matches(retained, attached))
        }
        _ => false,
    }
}

fn route_binding_matches(
    retained: &HirEntryRouteBinding,
    attached: &AttachedEntryRouteBinding,
) -> bool {
    name_matches(retained.parameter(), attached.parameter())
        && punctuation_matches(retained.assignment(), attached.equals())
        && punctuation_matches(retained.colon(), attached.colon())
        && name_matches(retained.path_capture(), attached.capture())
        && retained.has_trailing_recovery() == attached.has_trailing_recovery()
}

fn name_matches(retained: &HirRequiredName, attached: &AttachedEntryName) -> bool {
    match (retained, attached) {
        (HirRequiredName::Resolved(retained), AttachedEntryName::Authored { value, .. }) => {
            retained.as_str() == value.as_str()
        }
        (HirRequiredName::Missing, AttachedEntryName::Missing(_)) => true,
        _ => false,
    }
}

fn punctuation_matches(
    retained: HirEntryPunctuationState,
    attached: &AttachedEntryPunctuation,
) -> bool {
    retained
        == if attached.is_missing() {
            HirEntryPunctuationState::Missing
        } else {
            HirEntryPunctuationState::Present
        }
}

fn entry_expression_is_unallocated(
    attached: &AttachedEntryValue<AttachedExpressionNode>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
) -> bool {
    match attached {
        AttachedEntryValue::Authored(expression) | AttachedEntryValue::Recovered(expression) => {
            expression_tree_is_unallocated(expression, slots, &mut BTreeSet::new())
        }
        AttachedEntryValue::Missing(syntax) => {
            slots.prepared_source_owner::<ExprId>(syntax.id()).is_none()
        }
        AttachedEntryValue::Invalid(syntax) => parsed.attached_expression(syntax.id()).map_or_else(
            |_| slots.prepared_source_owner::<ExprId>(syntax.id()).is_none(),
            |expression| expression_tree_is_unallocated(&expression, slots, &mut BTreeSet::new()),
        ),
    }
}

fn option_expression_matches(
    retained: &HirEntryOptionValue,
    attached: &AttachedEntryValue<AttachedExpressionNode>,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    match (retained, attached) {
        (
            HirEntryOptionValue::Expression(retained),
            AttachedEntryValue::Authored(attached) | AttachedEntryValue::Recovered(attached),
        ) => expression_owner_matches(*retained, attached, item.scope(), slots, arenas),
        (HirEntryOptionValue::Missing, AttachedEntryValue::Missing(syntax)) => {
            slots.prepared_source_owner::<ExprId>(syntax.id()).is_none()
        }
        (HirEntryOptionValue::Invalid, AttachedEntryValue::Invalid(_)) => {
            entry_expression_is_unallocated(attached, parsed, slots)
        }
        _ => false,
    }
}

fn entry_item_state(
    attached: &AttachedEntryDeclaration,
    retained: &HirEntryDeclaration,
    item: &HirItem,
    slots: &SlotSnapshot,
    member_recovery: bool,
) -> HirItemPoisonState {
    item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| {
                matches!(attached.kind(), AttachedEntryKind::Missing(_))
                    .then_some(HirItemIssue::MissingKind)
            })
            .or_else(|| match attached.id() {
                AttachedEntryId::Missing(_) => Some(HirItemIssue::MissingId),
                AttachedEntryId::Authored {
                    canonical_entry_family: false,
                    ..
                } => Some(HirItemIssue::MalformedHeader),
                AttachedEntryId::Authored { .. } if retained.id().has_recovery() => {
                    Some(HirItemIssue::Recovery)
                }
                AttachedEntryId::Authored { .. } => None,
            })
            .or_else(|| {
                attached
                    .has_header_trailing_recovery()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                matches!(attached.body(), AttachedEntryBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or_else(|| member_recovery.then_some(HirItemIssue::InvalidMember))
            .or_else(|| {
                (!matches!(attached.body(), AttachedEntryBody::Missing(_))
                    && !attached.body().is_closed())
                .then_some(HirItemIssue::Recovery)
            }),
    )
}
