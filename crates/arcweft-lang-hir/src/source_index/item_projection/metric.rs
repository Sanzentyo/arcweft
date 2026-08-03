//! Metric-specific payload re-derivation for final item publication.

use arcweft_lang_syntax::attachment::{
    AttachedMetricBucketsValue, AttachedMetricDeclaration, AttachedMetricEntry, AttachedMetricKind,
    AttachedMetricLabelsBody, AttachedMetricUnitValue,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::expr::HirExprKind;
use crate::identity::{ExprId, ItemId};
use crate::item::{
    HirDeclarationMember, HirDeclarationMemberArena, HirDeclarationMemberId,
    HirDeclarationMemberIssue, HirDeclarationMemberKind, HirDeclarationMemberPoisonState, HirItem,
    HirItemFamily, HirItemIssue, HirItemKind, HirItemPoisonState, HirMetricAssignmentState,
    HirMetricBucketsValue, HirMetricKind, HirMetricKindIssue, HirMetricUnitValue,
};
use crate::leaf::{HirLiteral, HirStringLiteral};

use super::{
    ItemValidationArenas, expression_owner_matches, item_prefix_matches, item_state, prefix_issue,
    required_name_matches, retained_header_item_issue, retained_header_matches, slot_is_poisoned,
    source_matches, type_is_poisoned,
};

pub(super) fn payload_matches(
    owner: ItemId,
    attached: &AttachedMetricDeclaration,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Metric(metric) = item.kind() else {
        return false;
    };
    if !item_prefix_matches(item, attached.prefix(), slots)
        || !retained_header_matches(metric.header(), attached.header())
        || metric.kind() != metric_kind(attached.kind())
        || !type_matches(
            metric.value_type(),
            attached.value_type(),
            item.scope(),
            slots,
            arenas,
        )
    {
        return false;
    }

    let retained_members = match members {
        Some(members) if members.owner() == owner && members.family() == HirItemFamily::Metric => {
            members.members()
        }
        Some(_) => return false,
        None => &[],
    };
    let mut member_position = 0_usize;
    let mut expected_unit = None;
    let mut expected_labels = Vec::new();
    let mut expected_buckets = None;
    let mut first_body_issue = None;

    for (entry_position, entry) in attached.body().entries().iter().enumerate() {
        if usize::from(entry.source_ordinal()) != entry_position {
            return false;
        }
        let entry_has_issue = match entry {
            AttachedMetricEntry::Unit(attached) => {
                let Some((id, retained)) = next_member(owner, retained_members, member_position)
                else {
                    return false;
                };
                let HirDeclarationMemberKind::MetricUnit(unit) = retained.kind() else {
                    return false;
                };
                if retained.id() != id
                    || unit.assignment() != assignment_state(attached.assignment().is_missing())
                    || unit.is_duplicate() != attached.state().is_duplicate()
                    || !unit_value_matches(
                        unit.value(),
                        attached.value(),
                        item.scope(),
                        slots,
                        arenas,
                    )
                    || retained.state() != unit_state(attached, slots)
                {
                    return false;
                }
                expected_unit.get_or_insert(id);
                member_position += 1;
                attached.has_recovery() || retained.is_poisoned()
            }
            AttachedMetricEntry::Labels(attached) => {
                let mut issue = attached.state().has_recovery() || attached.body().has_recovery();
                if let AttachedMetricLabelsBody::Braced { labels, .. } = attached.body() {
                    for (label_position, attached) in labels.iter().enumerate() {
                        if usize::from(attached.source_ordinal()) != label_position {
                            return false;
                        }
                        let Some((id, retained)) =
                            next_member(owner, retained_members, member_position)
                        else {
                            return false;
                        };
                        let HirDeclarationMemberKind::MetricLabel(label) = retained.kind() else {
                            return false;
                        };
                        if retained.id() != id
                            || !required_name_matches(label.name(), attached.name())
                            || !type_matches(label.ty(), attached.ty(), item.scope(), slots, arenas)
                            || label.is_duplicate() != attached.is_duplicate()
                            || retained.state() != label_state(attached, label.ty(), slots)
                        {
                            return false;
                        }
                        issue |= retained.is_poisoned();
                        expected_labels.push(id);
                        member_position += 1;
                    }
                }
                issue
            }
            AttachedMetricEntry::Buckets(attached) => {
                let Some((id, retained)) = next_member(owner, retained_members, member_position)
                else {
                    return false;
                };
                let HirDeclarationMemberKind::MetricBuckets(buckets) = retained.kind() else {
                    return false;
                };
                if retained.id() != id
                    || buckets.assignment() != assignment_state(attached.assignment().is_missing())
                    || buckets.is_duplicate() != attached.state().is_duplicate()
                    || !buckets_value_matches(
                        buckets.value(),
                        attached.value(),
                        item.scope(),
                        slots,
                        arenas,
                    )
                    || retained.state() != buckets_state(attached, slots)
                {
                    return false;
                }
                expected_buckets.get_or_insert(id);
                member_position += 1;
                attached.has_recovery() || retained.is_poisoned()
            }
            AttachedMetricEntry::Recovery { .. } => true,
        };
        if entry_has_issue {
            first_body_issue.get_or_insert(HirItemIssue::InvalidMember);
        }
    }

    let all_members_match = member_position == retained_members.len()
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
    let arena_presence_matches = (member_position == 0) == members.is_none();
    let expected_state = metric_state(attached, item, first_body_issue, slots);

    all_members_match
        && arena_presence_matches
        && metric.unit() == expected_unit
        && metric.labels() == expected_labels.as_slice()
        && metric.buckets() == expected_buckets
        && item.state() == &expected_state
}

fn next_member<'a>(
    owner: ItemId,
    retained: &'a [HirDeclarationMember],
    position: usize,
) -> Option<(HirDeclarationMemberId, &'a HirDeclarationMember)> {
    let ordinal = u32::try_from(position).ok()?;
    Some((
        HirDeclarationMemberId::new(owner, ordinal),
        retained.get(position)?,
    ))
}

fn unit_value_matches(
    retained: &HirMetricUnitValue,
    attached: &AttachedMetricUnitValue,
    scope: crate::identity::ScopeId,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    match (retained, attached) {
        (HirMetricUnitValue::Missing, AttachedMetricUnitValue::Missing(_)) => true,
        (HirMetricUnitValue::NonString(retained), AttachedMetricUnitValue::NonString(attached)) => {
            expression_owner_matches(*retained, attached, scope, slots, arenas)
        }
        (
            HirMetricUnitValue::String(retained),
            AttachedMetricUnitValue::Decoded { expression, value },
        ) => {
            matches!(retained, HirStringLiteral::Value(actual) if actual == value)
                && string_expression_matches(retained, expression, scope, slots, arenas)
        }
        (
            HirMetricUnitValue::String(retained),
            AttachedMetricUnitValue::RecoveredString(expression),
        ) => {
            matches!(retained, HirStringLiteral::Invalid(_))
                && string_expression_matches(retained, expression, scope, slots, arenas)
        }
        _ => false,
    }
}

fn string_expression_matches(
    retained: &HirStringLiteral,
    attached: &arcweft_lang_syntax::attachment::AttachedExpressionNode,
    scope: crate::identity::ScopeId,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let Some(owner) = slots.prepared_source_owner::<ExprId>(attached.id()) else {
        return false;
    };
    expression_owner_matches(owner, attached, scope, slots, arenas)
        && arenas
            .expressions
            .resolve_prepared(slots, owner)
            .is_ok_and(|expression| {
                matches!(expression.kind(), HirExprKind::Literal(HirLiteral::String(actual)) if actual == retained)
            })
}

fn buckets_value_matches(
    retained: &HirMetricBucketsValue,
    attached: &AttachedMetricBucketsValue,
    scope: crate::identity::ScopeId,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    match (retained, attached) {
        (HirMetricBucketsValue::Missing, AttachedMetricBucketsValue::Missing(_)) => true,
        (
            HirMetricBucketsValue::NonSequence(retained),
            AttachedMetricBucketsValue::NonSequence(attached),
        ) => expression_owner_matches(*retained, attached, scope, slots, arenas),
        (
            HirMetricBucketsValue::Sequence(retained),
            AttachedMetricBucketsValue::Sequence(attached),
        ) => {
            let Some(owner) = slots.prepared_source_owner::<ExprId>(attached.id()) else {
                return false;
            };
            expression_owner_matches(owner, attached, scope, slots, arenas)
                && arenas
                    .expressions
                    .resolve_prepared(slots, owner)
                    .is_ok_and(|expression| {
                        matches!(expression.kind(), HirExprKind::BracketSequence(sequence) if sequence.elements() == retained.as_ref())
                    })
        }
        _ => false,
    }
}

fn type_matches(
    retained: crate::identity::TypeId,
    attached: &arcweft_lang_syntax::attachment::AttachedTypeRefNode,
    scope: crate::identity::ScopeId,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    source_matches(slots, retained, attached.id())
        && arenas
            .types
            .resolve_prepared(slots, retained)
            .is_ok_and(|ty| ty.scope() == scope)
}

fn unit_state(
    attached: &arcweft_lang_syntax::attachment::AttachedMetricUnitMember,
    slots: &crate::slot::SlotSnapshot,
) -> HirDeclarationMemberPoisonState {
    let issue = if attached.state().is_duplicate() {
        Some(HirDeclarationMemberIssue::Duplicate)
    } else if attached.assignment().is_missing() {
        Some(HirDeclarationMemberIssue::MissingAssignment)
    } else {
        match attached.value() {
            AttachedMetricUnitValue::Missing(_) => {
                Some(HirDeclarationMemberIssue::MissingInitializer)
            }
            AttachedMetricUnitValue::RecoveredString(_) | AttachedMetricUnitValue::NonString(_) => {
                Some(HirDeclarationMemberIssue::RecoveredChild)
            }
            AttachedMetricUnitValue::Decoded { expression, .. } => slots
                .prepared_source_owner::<ExprId>(expression.id())
                .is_some_and(|owner| slot_is_poisoned(slots, owner))
                .then_some(HirDeclarationMemberIssue::RecoveredChild),
        }
    };
    member_state(issue)
}

fn label_state(
    attached: &arcweft_lang_syntax::attachment::AttachedMetricLabel,
    ty: crate::identity::TypeId,
    slots: &crate::slot::SlotSnapshot,
) -> HirDeclarationMemberPoisonState {
    let issue = if attached.is_duplicate() {
        Some(HirDeclarationMemberIssue::Duplicate)
    } else if attached.name().is_missing()
        || attached.colon().is_missing()
        || type_is_poisoned(ty, slots)
    {
        Some(HirDeclarationMemberIssue::RecoveredChild)
    } else {
        None
    };
    member_state(issue)
}

fn buckets_state(
    attached: &arcweft_lang_syntax::attachment::AttachedMetricBucketsMember,
    slots: &crate::slot::SlotSnapshot,
) -> HirDeclarationMemberPoisonState {
    let issue = if attached.state().is_duplicate() {
        Some(HirDeclarationMemberIssue::Duplicate)
    } else if attached.assignment().is_missing() {
        Some(HirDeclarationMemberIssue::MissingAssignment)
    } else {
        match attached.value() {
            AttachedMetricBucketsValue::Missing(_) => {
                Some(HirDeclarationMemberIssue::MissingInitializer)
            }
            AttachedMetricBucketsValue::NonSequence(_) => {
                Some(HirDeclarationMemberIssue::RecoveredChild)
            }
            AttachedMetricBucketsValue::Sequence(expression) => (expression.children().is_empty()
                || slots
                    .prepared_source_owner::<ExprId>(expression.id())
                    .is_some_and(|owner| slot_is_poisoned(slots, owner)))
            .then_some(HirDeclarationMemberIssue::RecoveredChild),
        }
    };
    member_state(issue)
}

fn metric_state(
    attached: &AttachedMetricDeclaration,
    item: &HirItem,
    first_body_issue: Option<HirItemIssue>,
    slots: &crate::slot::SlotSnapshot,
) -> HirItemPoisonState {
    let value_type_poisoned = type_is_poisoned(
        match item.kind() {
            HirItemKind::Metric(metric) => metric.value_type(),
            _ => return item_state(Some(HirItemIssue::TransactionalChildFailure)),
        },
        slots,
    );
    item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| retained_header_item_issue(attached.header()))
            .or_else(|| {
                attached
                    .kind()
                    .has_recovery()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .colon()
                    .is_missing()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                value_type_poisoned.then_some(
                    if attached.value_type().syntax().kind() == SyntaxKind::MissingType {
                        HirItemIssue::MissingType
                    } else {
                        HirItemIssue::Recovery
                    },
                )
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
                (!attached.declaration_recoveries().is_empty()).then_some(HirItemIssue::Recovery)
            }),
    )
}

const fn metric_kind(attached: &AttachedMetricKind) -> HirMetricKind {
    match attached {
        AttachedMetricKind::Counter(_) => HirMetricKind::Counter,
        AttachedMetricKind::Gauge(_) => HirMetricKind::Gauge,
        AttachedMetricKind::Histogram(_) => HirMetricKind::Histogram,
        AttachedMetricKind::Missing(_) => HirMetricKind::Recovered(HirMetricKindIssue::Missing),
        AttachedMetricKind::Unknown(_) => HirMetricKind::Recovered(HirMetricKindIssue::Invalid),
    }
}

const fn assignment_state(missing: bool) -> HirMetricAssignmentState {
    if missing {
        HirMetricAssignmentState::Missing
    } else {
        HirMetricAssignmentState::Present
    }
}

const fn member_state(issue: Option<HirDeclarationMemberIssue>) -> HirDeclarationMemberPoisonState {
    match issue {
        Some(issue) => HirDeclarationMemberPoisonState::Poisoned(issue),
        None => HirDeclarationMemberPoisonState::Clean,
    }
}
