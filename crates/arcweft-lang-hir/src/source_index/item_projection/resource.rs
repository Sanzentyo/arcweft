//! Typed-resource payload re-derivation for final item publication.

use std::collections::BTreeSet;

use arcweft_lang_syntax::attachment::{
    AttachedResourceBody, AttachedResourceDeclaration, AttachedResourceInitializer,
    AttachedResourcePublicId,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::identity::ExprId;
use crate::item::{HirItem, HirItemIssue, HirItemKind, HirItemPoisonState};
use crate::slot::SlotSnapshot;

use super::{
    ItemValidationArenas, expression_owner_matches, expression_tree_is_unallocated,
    item_prefix_matches, item_state, name_issue, prefix_issue, required_name_matches,
    slot_is_poisoned, type_is_poisoned, type_owner_matches,
};

pub(super) fn payload_matches(
    attached: &AttachedResourceDeclaration,
    item: &HirItem,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Resource(retained) = item.kind() else {
        return false;
    };
    let attached_fields = attached
        .body()
        .fields()
        .iter()
        .filter(|field| !matches!(field.initializer(), AttachedResourceInitializer::Absent))
        .collect::<Vec<_>>();
    let public_id_matches = match (retained.public_id(), attached.public_id()) {
        (None, AttachedResourcePublicId::Absent | AttachedResourcePublicId::Recovered { .. }) => {
            true
        }
        (Some(retained), AttachedResourcePublicId::Explicit { value, .. }) => retained == value,
        _ => false,
    };
    let public_id_is_unallocated = attached
        .public_id()
        .syntax()
        .is_none_or(|syntax| expression_tree_is_unallocated(syntax, slots, &mut BTreeSet::new()));
    item_prefix_matches(item, attached.prefix(), slots)
        && public_id_matches
        && public_id_is_unallocated
        && required_name_matches(retained.name(), attached.name())
        && type_owner_matches(retained.resource_type(), attached.resource_type(), slots)
        && retained.fields().len() == attached_fields.len()
        && retained
            .fields()
            .iter()
            .zip(attached_fields)
            .all(|(retained, attached)| {
                required_name_matches(retained.name(), attached.name())
                    && attached
                        .initializer()
                        .authored()
                        .is_some_and(|initializer| {
                            expression_owner_matches(
                                retained.value(),
                                initializer,
                                item.scope(),
                                slots,
                                arenas,
                            )
                        })
            })
        && item.members().is_empty()
        && item.state() == &resource_item_state(attached, retained, item.prefix(), slots)
}

fn resource_item_state(
    attached: &AttachedResourceDeclaration,
    retained: &crate::item::HirResourceDeclaration,
    prefix: &crate::item::HirItemPrefix,
    slots: &SlotSnapshot,
) -> HirItemPoisonState {
    let type_issue = if type_is_poisoned(retained.resource_type(), slots) {
        Some(
            if attached.resource_type().syntax().kind() == SyntaxKind::MissingType {
                HirItemIssue::MissingType
            } else {
                HirItemIssue::Recovery
            },
        )
    } else if attached.has_nominal_type_head() {
        None
    } else {
        Some(HirItemIssue::MalformedHeader)
    };
    let mut retained_fields = retained.fields().iter();
    let field_issue = attached.body().fields().iter().any(|attached| {
        let initializer = match attached.initializer() {
            AttachedResourceInitializer::Authored(initializer) => initializer,
            AttachedResourceInitializer::Absent => {
                return true;
            }
        };
        let Some(retained) = retained_fields.next() else {
            return true;
        };
        attached.has_recovery()
            || retained.name().is_recovered()
            || slot_is_poisoned(slots, retained.value())
            || slots
                .prepared_source_owner::<ExprId>(initializer.id())
                .is_none_or(|owner| owner != retained.value())
    }) || retained_fields.next().is_some();
    item_state(
        prefix_issue(attached.prefix(), prefix, slots)
            .or_else(|| name_issue(attached.name()))
            .or_else(|| {
                attached
                    .public_id()
                    .has_recovery()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .colon()
                    .is_missing()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or(type_issue)
            .or_else(|| {
                matches!(attached.body(), AttachedResourceBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or_else(|| field_issue.then_some(HirItemIssue::InvalidMember))
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            }),
    )
}
