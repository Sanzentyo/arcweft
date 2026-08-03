//! Temporary Source payload re-derivation for final item publication.
//!
//! This validator consumes only the parser-owned attached projection. It does
//! not consult the detached Source AST or the clone-bearing legacy HIR that
//! Lang-01.3 removes.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedExpressionNode, AttachedPatternChild, AttachedPatternNode,
    AttachedSourceBackpressurePolicy, AttachedSourceBody, AttachedSourceBoundedArgument,
    AttachedSourceDeclaration, AttachedSourceExpression, AttachedSourceHandlerBody,
    AttachedSourceHandlerEvent, AttachedSourceId, AttachedSourceMember, AttachedSourceName,
    AttachedSourceOverflowPolicy, AttachedSourcePattern, AttachedSourcePrivacyPolicy,
    AttachedSourcePunctuation, AttachedSourceReplayPolicy,
};
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::identity::{ExprId, ItemId, LocalGeneration, LocalId, PatternId, ScopeId, StmtId};
use crate::item::{
    HirItem, HirItemIssue, HirItemKind, HirItemPoisonState, HirRequiredName,
    HirSourceBackpressurePolicy, HirSourceBackpressureValue, HirSourceBoundedArgument,
    HirSourceChildState, HirSourceEventIssue, HirSourceEventPattern, HirSourceExpressionValue,
    HirSourceHandler, HirSourceHandlerBody, HirSourceId, HirSourceItem, HirSourceOverflowPolicy,
    HirSourceOverflowValue, HirSourcePatternValue, HirSourcePolicyBinding, HirSourcePolicyIssue,
    HirSourcePrivacyPolicy, HirSourcePrivacyValue, HirSourcePunctuationState,
    HirSourceReplayPolicy, HirSourceReplayValue, HirSourceRequiredSlot,
};
use crate::leaf::HirName;
use crate::scope::{HirPatternBindingPolicy, HirScopeKind, HirScopeOwner};
use crate::slot::SlotSnapshot;

use super::super::block_projection::{
    AttachedStatementBlock, BlockValidationArenas, ItemStatementBlockRetained,
    item_statement_block_matches_with_prefix,
};
use super::super::control_projection::canonical_pattern_locals;
use super::super::pattern_projection::{BindingLocalValidation, binding_locals_match};
use super::{
    ItemValidationArenas, expression_owner_matches, expression_tree_is_unallocated,
    item_prefix_matches, item_state, prefix_issue, slot_is_poisoned, source_matches,
    type_owner_matches, type_tree_is_unallocated,
};

#[derive(Default)]
struct HeaderCounts {
    from: usize,
    backpressure: usize,
    replay: usize,
    privacy: usize,
}

struct BodyEvidence {
    issue: Option<HirItemIssue>,
}

struct HandlerEventEvidence {
    locals: Vec<LocalId>,
    generations: BTreeMap<HirName, LocalGeneration>,
    poisoned: bool,
}

pub(super) fn payload_matches(
    owner: ItemId,
    attached: &AttachedSourceDeclaration,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Source(retained) = item.kind() else {
        return false;
    };
    let Some(body) = body_matches(owner, retained, attached, item, parsed, slots, arenas) else {
        return false;
    };
    let type_matches =
        type_owner_matches(retained.source_type(), attached.source_type().node(), slots)
            && arenas
                .types
                .resolve_prepared(slots, retained.source_type())
                .is_ok_and(|ty| ty.scope() == item.scope());
    item_prefix_matches(item, attached.prefix(), slots)
        && id_matches(retained.id(), attached.id(), slots)
        && name_matches(retained.name(), attached.name())
        && type_matches
        && item.members().is_empty()
        && item.state() == &expected_item_state(attached, retained, item, slots, body.issue)
}

fn body_matches(
    owner: ItemId,
    retained: &HirSourceItem,
    attached: &AttachedSourceDeclaration,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<BodyEvidence> {
    match attached.body() {
        AttachedSourceBody::Missing(_) => {
            (matches!(retained.body(), crate::item::HirSourceBody::Missing)
                && retained.handlers().is_empty()
                && matches!(retained.from(), HirSourceRequiredSlot::Missing)
                && matches!(retained.backpressure(), HirSourceRequiredSlot::Missing)
                && matches!(retained.replay(), HirSourceRequiredSlot::Missing)
                && matches!(retained.privacy(), HirSourceRequiredSlot::Missing))
            .then_some(BodyEvidence { issue: None })
        }
        AttachedSourceBody::Braced { members, .. } => {
            if !matches!(
                retained.body(),
                crate::item::HirSourceBody::Braced { closed }
                    if closed == attached.body().is_closed()
            ) {
                return None;
            }

            let mut counts = HeaderCounts::default();
            let mut handler_index = 0usize;
            let mut issue = None;
            for (ordinal, member) in members.iter().enumerate() {
                if member.source_ordinal() != u32::try_from(ordinal).ok()? {
                    return None;
                }
                match member {
                    AttachedSourceMember::From {
                        value, duplicate, ..
                    } => {
                        let later = counts.from != 0;
                        if *duplicate != later {
                            return None;
                        }
                        counts.from += 1;
                        if later {
                            if !source_expression_is_unallocated(value, slots) {
                                return None;
                            }
                            issue.get_or_insert(HirItemIssue::InvalidMember);
                        } else {
                            let HirSourceRequiredSlot::Authored {
                                value: retained_value,
                                ..
                            } = retained.from()
                            else {
                                return None;
                            };
                            let poisoned = source_expression_matches(
                                retained_value,
                                value,
                                item.scope(),
                                slots,
                                arenas,
                            )?;
                            if value.has_recovery() || poisoned {
                                issue.get_or_insert(HirItemIssue::InvalidMember);
                            }
                        }
                    }
                    AttachedSourceMember::Backpressure {
                        assignment,
                        policy,
                        duplicate,
                        ..
                    } => {
                        let later = counts.backpressure != 0;
                        if *duplicate != later {
                            return None;
                        }
                        counts.backpressure += 1;
                        if later {
                            if !backpressure_is_unallocated(policy, slots) {
                                return None;
                            }
                            issue.get_or_insert(HirItemIssue::InvalidMember);
                        } else {
                            let HirSourceRequiredSlot::Authored {
                                value: retained_value,
                                ..
                            } = retained.backpressure()
                            else {
                                return None;
                            };
                            let poisoned = backpressure_matches(
                                retained_value,
                                assignment,
                                policy,
                                item.scope(),
                                slots,
                                arenas,
                            )?;
                            if assignment.is_missing() || policy.has_recovery() || poisoned {
                                issue.get_or_insert(HirItemIssue::InvalidMember);
                            }
                        }
                    }
                    AttachedSourceMember::Replay {
                        assignment,
                        policy,
                        duplicate,
                        ..
                    } => {
                        let later = counts.replay != 0;
                        if *duplicate != later {
                            return None;
                        }
                        counts.replay += 1;
                        if later {
                            if !source_expression_is_unallocated(policy.expression(), slots) {
                                return None;
                            }
                            issue.get_or_insert(HirItemIssue::InvalidMember);
                        } else {
                            let HirSourceRequiredSlot::Authored {
                                value: retained_value,
                                ..
                            } = retained.replay()
                            else {
                                return None;
                            };
                            if !named_policy_binding_matches(
                                retained_value,
                                assignment,
                                &replay_value(policy),
                            ) || !source_expression_is_unallocated(policy.expression(), slots)
                            {
                                return None;
                            }
                            if assignment.is_missing() || policy.has_recovery() {
                                issue.get_or_insert(HirItemIssue::InvalidMember);
                            }
                        }
                    }
                    AttachedSourceMember::Privacy {
                        assignment,
                        policy,
                        duplicate,
                        ..
                    } => {
                        let later = counts.privacy != 0;
                        if *duplicate != later {
                            return None;
                        }
                        counts.privacy += 1;
                        if later {
                            if !source_expression_is_unallocated(policy.expression(), slots) {
                                return None;
                            }
                            issue.get_or_insert(HirItemIssue::InvalidMember);
                        } else {
                            let HirSourceRequiredSlot::Authored {
                                value: retained_value,
                                ..
                            } = retained.privacy()
                            else {
                                return None;
                            };
                            if !named_policy_binding_matches(
                                retained_value,
                                assignment,
                                &privacy_value(policy),
                            ) || !source_expression_is_unallocated(policy.expression(), slots)
                            {
                                return None;
                            }
                            if assignment.is_missing() || policy.has_recovery() {
                                issue.get_or_insert(HirItemIssue::InvalidMember);
                            }
                        }
                    }
                    AttachedSourceMember::Handler {
                        syntax,
                        event,
                        arrow,
                        body,
                        ..
                    } => {
                        let retained_handler = retained.handlers().get(handler_index)?;
                        handler_index += 1;
                        let recovered = handler_matches(
                            owner,
                            retained_handler,
                            syntax.id(),
                            event,
                            arrow,
                            body,
                            item.scope(),
                            parsed,
                            slots,
                            arenas,
                        )?;
                        if slots.prepared_source_owner::<StmtId>(syntax.id()).is_some() {
                            return None;
                        }
                        if recovered {
                            issue.get_or_insert(HirItemIssue::InvalidMember);
                        }
                    }
                    AttachedSourceMember::UnsupportedContract {
                        syntax, condition, ..
                    } => {
                        let syntax_id = match syntax {
                            arcweft_lang_syntax::attachment::AttachedSourceContract::Requires(
                                syntax,
                            ) => syntax.id(),
                            arcweft_lang_syntax::attachment::AttachedSourceContract::Ensures(
                                syntax,
                            ) => syntax.id(),
                        };
                        if slots.prepared_source_owner::<StmtId>(syntax_id).is_some()
                            || !source_expression_is_unallocated(condition, slots)
                        {
                            return None;
                        }
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                    }
                    AttachedSourceMember::Recovery { syntax, .. } => {
                        if slots.prepared_source_owner::<StmtId>(syntax.id()).is_some() {
                            return None;
                        }
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                    }
                }
            }

            if handler_index != retained.handlers().len()
                || !required_slot_shape_matches(retained.from(), counts.from)
                || !required_slot_shape_matches(retained.backpressure(), counts.backpressure)
                || !required_slot_shape_matches(retained.replay(), counts.replay)
                || !required_slot_shape_matches(retained.privacy(), counts.privacy)
                || !handler_scope_inventory_matches(owner, retained, slots, arenas)
            {
                return None;
            }
            if counts.from == 0
                || counts.backpressure == 0
                || counts.replay == 0
                || counts.privacy == 0
            {
                issue.get_or_insert(HirItemIssue::InvalidMember);
            }
            Some(BodyEvidence { issue })
        }
    }
}

fn id_matches(
    retained: Option<&HirSourceId>,
    attached: &AttachedSourceId,
    slots: &SlotSnapshot,
) -> bool {
    match (retained, attached) {
        (None, AttachedSourceId::Absent) => true,
        (
            Some(retained),
            AttachedSourceId::Authored {
                syntax,
                reference,
                canonical_source_family,
                requires_name,
            },
        ) => {
            retained.is_canonical_source_family() == *canonical_source_family
                && retained.requires_name() == *requires_name
                && crate::final_lowering::id_ref_projection::id_ref(reference)
                    .is_ok_and(|value| &value == retained.value())
                && slots.prepared_source_owner::<ExprId>(syntax.id()).is_none()
        }
        _ => false,
    }
}

fn name_matches(retained: Option<&HirRequiredName>, attached: &AttachedSourceName) -> bool {
    match (retained, attached) {
        (None, AttachedSourceName::Absent)
        | (Some(HirRequiredName::Missing), AttachedSourceName::Missing(_))
        | (Some(HirRequiredName::Invalid), AttachedSourceName::Authored { value: Err(_), .. }) => {
            true
        }
        (
            Some(HirRequiredName::Resolved(retained)),
            AttachedSourceName::Authored {
                value: Ok(attached),
                ..
            },
        ) => retained.as_str() == attached.as_str(),
        _ => false,
    }
}

fn required_slot_shape_matches<T>(retained: &HirSourceRequiredSlot<T>, count: usize) -> bool {
    match retained {
        HirSourceRequiredSlot::Authored { duplicate, .. } => {
            count != 0 && *duplicate == (count > 1)
        }
        HirSourceRequiredSlot::Missing => count == 0,
    }
}

fn source_expression_matches(
    retained: &HirSourceExpressionValue,
    attached: &AttachedSourceExpression,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    match (retained, attached) {
        (
            HirSourceExpressionValue::Expression(retained),
            AttachedSourceExpression::Authored(attached),
        ) => expression_owner_matches(*retained, attached, scope, slots, arenas)
            .then(|| slot_is_poisoned(slots, *retained)),
        (HirSourceExpressionValue::Invalid, AttachedSourceExpression::Recovered(_))
        | (HirSourceExpressionValue::Missing, AttachedSourceExpression::Missing(_)) => {
            source_expression_is_unallocated(attached, slots).then_some(false)
        }
        _ => None,
    }
}

fn backpressure_matches(
    retained: &HirSourcePolicyBinding<HirSourceBackpressureValue>,
    assignment: &AttachedSourcePunctuation,
    attached: &AttachedSourceBackpressurePolicy,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    if retained.assignment() != punctuation(assignment) {
        return None;
    }
    match (retained.value(), attached) {
        (
            HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Bounded {
                capacity: retained_capacity,
                overflow: retained_overflow,
                unexpected_arguments: retained_unexpected,
                recovered_call: retained_recovered,
            }),
            AttachedSourceBackpressurePolicy::Bounded {
                expression,
                capacity,
                overflow,
                unexpected_arguments,
                recovered_call,
            },
        ) => {
            let poisoned = capacity_matches(retained_capacity, capacity, scope, slots, arenas)?;
            if retained_unexpected != unexpected_arguments
                || retained_recovered != recovered_call
                || !overflow_matches(retained_overflow, overflow)
                || !bounded_outer_allocations_match(expression, capacity, slots)
            {
                return None;
            }
            Some(poisoned)
        }
        (retained, AttachedSourceBackpressurePolicy::Latest(expression)) => {
            (backpressure_known_value_matches(
                retained,
                expression,
                HirSourceBackpressurePolicy::Latest,
            ) && source_expression_is_unallocated(expression, slots))
            .then_some(false)
        }
        (retained, AttachedSourceBackpressurePolicy::BlockingNotAllowed(expression)) => {
            (backpressure_known_value_matches(
                retained,
                expression,
                HirSourceBackpressurePolicy::BlockingNotAllowed,
            ) && source_expression_is_unallocated(expression, slots))
            .then_some(false)
        }
        (
            HirSourceBackpressureValue::Recovered {
                authored: None,
                issue: HirSourcePolicyIssue::Missing,
            },
            AttachedSourceBackpressurePolicy::Missing(expression),
        )
        | (
            HirSourceBackpressureValue::Recovered {
                authored: None,
                issue: HirSourcePolicyIssue::Invalid,
            },
            AttachedSourceBackpressurePolicy::Invalid(expression),
        ) => source_expression_is_unallocated(expression, slots).then_some(false),
        (
            HirSourceBackpressureValue::Recovered { authored, issue },
            AttachedSourceBackpressurePolicy::Unknown { expression, value },
        ) => (recovered_name_matches(authored.as_ref(), *issue, value.as_ref())
            && source_expression_is_unallocated(expression, slots))
        .then_some(false),
        _ => None,
    }
}

fn backpressure_known_value_matches(
    retained: &HirSourceBackpressureValue,
    attached: &AttachedSourceExpression,
    expected: HirSourceBackpressurePolicy,
) -> bool {
    match attached {
        AttachedSourceExpression::Authored(_) => {
            retained == &HirSourceBackpressureValue::Resolved(expected)
        }
        AttachedSourceExpression::Recovered(_) => matches!(
            retained,
            HirSourceBackpressureValue::Recovered {
                authored: None,
                issue: HirSourcePolicyIssue::Invalid,
            }
        ),
        AttachedSourceExpression::Missing(_) => matches!(
            retained,
            HirSourceBackpressureValue::Recovered {
                authored: None,
                issue: HirSourcePolicyIssue::Missing,
            }
        ),
    }
}

fn capacity_matches(
    retained: &HirSourceBoundedArgument<HirSourceExpressionValue>,
    attached: &AttachedSourceBoundedArgument,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    match attached {
        AttachedSourceBoundedArgument::Missing => {
            (retained.value() == &HirSourceExpressionValue::Missing && !retained.is_duplicate())
                .then_some(false)
        }
        AttachedSourceBoundedArgument::Present {
            value, duplicate, ..
        } => {
            if retained.is_duplicate() != *duplicate {
                return None;
            }
            source_expression_matches(retained.value(), value, scope, slots, arenas)
        }
    }
}

fn overflow_matches(
    retained: &HirSourceBoundedArgument<HirSourceOverflowValue>,
    attached: &AttachedSourceOverflowPolicy,
) -> bool {
    let (value, duplicate) = match attached {
        AttachedSourceOverflowPolicy::DropOldest(argument) => (
            overflow_known_value(argument, HirSourceOverflowPolicy::DropOldest),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::DropNewest(argument) => (
            overflow_known_value(argument, HirSourceOverflowPolicy::DropNewest),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::Error(argument) => (
            overflow_known_value(argument, HirSourceOverflowPolicy::Error),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::Coalesce(argument) => (
            overflow_known_value(argument, HirSourceOverflowPolicy::Coalesce),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::Missing => (
            HirSourceOverflowValue::Recovered {
                authored: None,
                issue: HirSourcePolicyIssue::Missing,
            },
            false,
        ),
        AttachedSourceOverflowPolicy::Unknown { argument, value } => (
            recovered_overflow_name(value.as_ref()),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::Invalid(argument) => (
            HirSourceOverflowValue::Recovered {
                authored: None,
                issue: HirSourcePolicyIssue::Invalid,
            },
            argument.is_duplicate(),
        ),
    };
    retained.value() == &value && retained.is_duplicate() == duplicate
}

fn overflow_known_value(
    argument: &AttachedSourceBoundedArgument,
    policy: HirSourceOverflowPolicy,
) -> HirSourceOverflowValue {
    match argument.value() {
        Some(AttachedSourceExpression::Authored(_)) => HirSourceOverflowValue::Resolved(policy),
        Some(AttachedSourceExpression::Recovered(_)) => HirSourceOverflowValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
        Some(AttachedSourceExpression::Missing(_)) | None => HirSourceOverflowValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
    }
}

fn recovered_overflow_name(
    attached: Option<&arcweft_lang_syntax::name::SyntaxName>,
) -> HirSourceOverflowValue {
    match attached {
        Some(attached) => HirSourceOverflowValue::Recovered {
            authored: HirName::try_new(attached.as_str().into()).ok(),
            issue: HirSourcePolicyIssue::Unsupported,
        },
        None => HirSourceOverflowValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    }
}

fn replay_value(attached: &AttachedSourceReplayPolicy) -> HirSourceReplayValue {
    match attached {
        AttachedSourceReplayPolicy::Full(expression) => {
            replay_known_value(expression, HirSourceReplayPolicy::Full)
        }
        AttachedSourceReplayPolicy::HashOnly(expression) => {
            replay_known_value(expression, HirSourceReplayPolicy::HashOnly)
        }
        AttachedSourceReplayPolicy::Summary(expression) => {
            replay_known_value(expression, HirSourceReplayPolicy::Summary)
        }
        AttachedSourceReplayPolicy::EventOnly(expression) => {
            replay_known_value(expression, HirSourceReplayPolicy::EventOnly)
        }
        AttachedSourceReplayPolicy::None(expression) => {
            replay_known_value(expression, HirSourceReplayPolicy::None)
        }
        AttachedSourceReplayPolicy::Missing(_) => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
        AttachedSourceReplayPolicy::Unknown { value, .. } => {
            let (authored, issue) = recovered_name(value.as_ref());
            HirSourceReplayValue::Recovered { authored, issue }
        }
        AttachedSourceReplayPolicy::Invalid(_) => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    }
}

fn replay_known_value(
    attached: &AttachedSourceExpression,
    policy: HirSourceReplayPolicy,
) -> HirSourceReplayValue {
    match attached {
        AttachedSourceExpression::Authored(_) => HirSourceReplayValue::Resolved(policy),
        AttachedSourceExpression::Recovered(_) => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
        AttachedSourceExpression::Missing(_) => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
    }
}

fn privacy_value(attached: &AttachedSourcePrivacyPolicy) -> HirSourcePrivacyValue {
    match attached {
        AttachedSourcePrivacyPolicy::Transient(expression) => {
            privacy_known_value(expression, HirSourcePrivacyPolicy::Transient)
        }
        AttachedSourcePrivacyPolicy::Redacted(expression) => {
            privacy_known_value(expression, HirSourcePrivacyPolicy::Redacted)
        }
        AttachedSourcePrivacyPolicy::Recordable(expression) => {
            privacy_known_value(expression, HirSourcePrivacyPolicy::Recordable)
        }
        AttachedSourcePrivacyPolicy::Private(expression) => {
            privacy_known_value(expression, HirSourcePrivacyPolicy::Private)
        }
        AttachedSourcePrivacyPolicy::Missing(_) => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
        AttachedSourcePrivacyPolicy::Unknown { value, .. } => {
            let (authored, issue) = recovered_name(value.as_ref());
            HirSourcePrivacyValue::Recovered { authored, issue }
        }
        AttachedSourcePrivacyPolicy::Invalid(_) => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    }
}

fn privacy_known_value(
    attached: &AttachedSourceExpression,
    policy: HirSourcePrivacyPolicy,
) -> HirSourcePrivacyValue {
    match attached {
        AttachedSourceExpression::Authored(_) => HirSourcePrivacyValue::Resolved(policy),
        AttachedSourceExpression::Recovered(_) => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
        AttachedSourceExpression::Missing(_) => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
    }
}

fn named_policy_binding_matches<T: PartialEq>(
    retained: &HirSourcePolicyBinding<T>,
    assignment: &AttachedSourcePunctuation,
    expected: &T,
) -> bool {
    retained.assignment() == punctuation(assignment) && retained.value() == expected
}

fn recovered_name(
    attached: Option<&arcweft_lang_syntax::name::SyntaxName>,
) -> (Option<HirName>, HirSourcePolicyIssue) {
    match attached {
        Some(attached) => (
            HirName::try_new(attached.as_str().into()).ok(),
            HirSourcePolicyIssue::Unsupported,
        ),
        None => (None, HirSourcePolicyIssue::Invalid),
    }
}

fn recovered_name_matches(
    retained: Option<&HirName>,
    issue: HirSourcePolicyIssue,
    attached: Option<&arcweft_lang_syntax::name::SyntaxName>,
) -> bool {
    let (expected, expected_issue) = recovered_name(attached);
    issue == expected_issue && retained == expected.as_ref()
}

fn handler_matches(
    owner: ItemId,
    retained: &HirSourceHandler,
    handler_syntax: arcweft_lang_syntax::attachment::SyntaxNodeId,
    event: &AttachedSourceHandlerEvent,
    arrow: &AttachedSourcePunctuation,
    body: &AttachedSourceHandlerBody,
    parent_scope: ScopeId,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    if retained.arrow() != punctuation(arrow) {
        return None;
    }
    let mut event = event_matches(retained.event(), event, retained.scope(), slots, arenas)?;
    let (retained_statements, attached_block) = match (retained.body(), body) {
        (HirSourceHandlerBody::Missing, AttachedSourceHandlerBody::Missing(syntax)) => (
            &[][..],
            AttachedStatementBlock {
                id: syntax.id(),
                source: syntax.source_span(),
                statements: &[],
            },
        ),
        (
            HirSourceHandlerBody::Statement(statement),
            AttachedSourceHandlerBody::Statement(attached),
        ) => (
            std::slice::from_ref(statement),
            AttachedStatementBlock {
                id: attached.id(),
                source: attached.source_span(),
                statements: std::slice::from_ref(attached),
            },
        ),
        (
            HirSourceHandlerBody::Block { statements, closed },
            AttachedSourceHandlerBody::Block {
                syntax,
                statements: attached,
                closed: attached_closed,
            },
        ) if closed == attached_closed => (
            statements.as_ref(),
            AttachedStatementBlock {
                id: syntax.id(),
                source: syntax.source_span(),
                statements: attached,
            },
        ),
        _ => return None,
    };
    let statement_recovery = item_statement_block_matches_with_prefix(
        parsed,
        slots,
        &BlockValidationArenas {
            expressions: arenas.expressions,
            statements: arenas.statements,
            scopes: arenas.scopes,
            locals: arenas.locals,
            patterns: arenas.patterns,
        },
        ItemStatementBlockRetained {
            owner,
            parent_scope,
            scope: retained.scope(),
            statements: retained_statements,
        },
        attached_block,
        &event.locals,
        &mut event.generations,
    )?;
    let handler_is_unallocated = slots
        .prepared_source_owner::<StmtId>(handler_syntax)
        .is_none();
    handler_is_unallocated
        .then_some(retained.has_recovery() || event.poisoned || statement_recovery)
}

fn event_matches(
    retained: &HirSourceEventPattern,
    attached: &AttachedSourceHandlerEvent,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<HandlerEventEvidence> {
    match (retained, attached) {
        (HirSourceEventPattern::Item(retained), AttachedSourceHandlerEvent::Item(attached))
        | (HirSourceEventPattern::Error(retained), AttachedSourceHandlerEvent::Error(attached))
        | (
            HirSourceEventPattern::Progress(retained),
            AttachedSourceHandlerEvent::Progress(attached),
        ) => pattern_event_matches(retained, attached, scope, slots, arenas),
        (
            HirSourceEventPattern::Disconnected(retained),
            AttachedSourceHandlerEvent::Disconnected(attached),
        )
        | (
            HirSourceEventPattern::PermissionRevoked(retained),
            AttachedSourceHandlerEvent::PermissionRevoked(attached),
        )
        | (HirSourceEventPattern::End(retained), AttachedSourceHandlerEvent::End(attached)) => {
            (child_state_matches(*retained, attached)
                && source_expression_is_unallocated(attached, slots))
            .then_some(HandlerEventEvidence {
                locals: Vec::new(),
                generations: BTreeMap::new(),
                poisoned: false,
            })
        }
        (
            HirSourceEventPattern::Recovered {
                authored,
                condition,
                issue,
            },
            AttachedSourceHandlerEvent::Unknown {
                value,
                condition: attached_condition,
            },
        ) => {
            let expected_issue = if value.is_some() {
                HirSourceEventIssue::Unsupported
            } else if matches!(attached_condition, AttachedSourceExpression::Missing(_)) {
                HirSourceEventIssue::Missing
            } else {
                HirSourceEventIssue::Invalid
            };
            let name_matches = match (authored, value) {
                (Some(retained), Some(attached)) => retained.as_str() == attached.as_str(),
                (None, None) => true,
                _ => false,
            };
            (name_matches
                && *issue == expected_issue
                && child_state_matches(*condition, attached_condition)
                && source_expression_is_unallocated(attached_condition, slots))
            .then_some(HandlerEventEvidence {
                locals: Vec::new(),
                generations: BTreeMap::new(),
                poisoned: false,
            })
        }
        _ => None,
    }
}

fn pattern_event_matches(
    retained: &HirSourcePatternValue,
    attached: &AttachedSourcePattern,
    scope: ScopeId,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<HandlerEventEvidence> {
    match (retained, attached) {
        (HirSourcePatternValue::Pattern(owner), AttachedSourcePattern::Authored(attached)) => {
            if !source_matches(slots, *owner, attached.id())
                || !arenas
                    .patterns
                    .resolve_prepared(slots, *owner)
                    .is_ok_and(|pattern| pattern.scope() == scope)
            {
                return None;
            }
            let block_arenas = BlockValidationArenas {
                expressions: arenas.expressions,
                statements: arenas.statements,
                scopes: arenas.scopes,
                locals: arenas.locals,
                patterns: arenas.patterns,
            };
            let expected = canonical_pattern_locals(slots, &block_arenas, *owner, *owner, scope)?;
            let locals = expected
                .iter()
                .map(|expected| expected.local)
                .collect::<Vec<_>>();
            let mut generations = BTreeMap::new();
            let poisoned = {
                let mut validation = BindingLocalValidation::new(
                    scope,
                    HirPatternBindingPolicy::PatternBinding,
                    &mut generations,
                    slots,
                    arenas.patterns,
                    arenas.locals,
                );
                if !binding_locals_match(attached, &expected, &mut validation) {
                    return None;
                }
                validation.is_poisoned() || slot_is_poisoned(slots, *owner)
            };
            Some(HandlerEventEvidence {
                locals,
                generations,
                poisoned,
            })
        }
        (HirSourcePatternValue::Invalid, AttachedSourcePattern::Recovered(attached))
        | (HirSourcePatternValue::Missing, AttachedSourcePattern::Missing(attached)) => {
            pattern_tree_is_unallocated(attached, slots).then_some(HandlerEventEvidence {
                locals: Vec::new(),
                generations: BTreeMap::new(),
                poisoned: false,
            })
        }
        _ => None,
    }
}

fn handler_scope_inventory_matches(
    owner: ItemId,
    retained: &HirSourceItem,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let expected = retained
        .handlers()
        .iter()
        .map(HirSourceHandler::scope)
        .collect::<BTreeSet<_>>();
    if expected.len() != retained.handlers().len() {
        return false;
    }
    let Ok(scopes) = arenas.scopes.try_iter_prepared(slots) else {
        return false;
    };
    let actual = scopes
        .into_iter()
        .filter_map(|(scope, payload)| {
            (payload.owner() == &HirScopeOwner::Item(owner)
                && payload.kind() == HirScopeKind::Block)
                .then_some(scope)
        })
        .collect::<BTreeSet<_>>();
    actual == expected
}

fn source_expression_is_unallocated(
    attached: &AttachedSourceExpression,
    slots: &SlotSnapshot,
) -> bool {
    match attached {
        AttachedSourceExpression::Authored(expression)
        | AttachedSourceExpression::Recovered(expression) => {
            expression_tree_is_unallocated(expression, slots, &mut BTreeSet::new())
        }
        AttachedSourceExpression::Missing(syntax) => {
            slots.prepared_source_owner::<ExprId>(syntax.id()).is_none()
        }
    }
}

fn backpressure_is_unallocated(
    attached: &AttachedSourceBackpressurePolicy,
    slots: &SlotSnapshot,
) -> bool {
    source_expression_is_unallocated(attached.expression(), slots)
}

fn bounded_outer_allocations_match(
    outer: &AttachedSourceExpression,
    capacity: &AttachedSourceBoundedArgument,
    slots: &SlotSnapshot,
) -> bool {
    let mut allowed = BTreeSet::new();
    if let Some(AttachedSourceExpression::Authored(capacity)) = capacity.value()
        && !collect_expression_syntax(capacity, &mut allowed)
    {
        return false;
    }
    match outer {
        AttachedSourceExpression::Authored(expression)
        | AttachedSourceExpression::Recovered(expression) => {
            expression_allocations_are_subset(expression, &allowed, slots, &mut BTreeSet::new())
        }
        AttachedSourceExpression::Missing(syntax) => {
            allowed.is_empty() && slots.prepared_source_owner::<ExprId>(syntax.id()).is_none()
        }
    }
}

fn collect_expression_syntax(
    attached: &AttachedExpressionNode,
    collected: &mut BTreeSet<arcweft_lang_syntax::attachment::SyntaxNodeId>,
) -> bool {
    if !collected.insert(attached.id()) {
        return false;
    }
    attached.children().iter().all(|child| {
        child.authored_semantic().is_ok_and(|child| {
            child.is_none_or(|child| collect_expression_syntax(&child, collected))
        })
    })
}

fn expression_allocations_are_subset(
    attached: &AttachedExpressionNode,
    allowed: &BTreeSet<arcweft_lang_syntax::attachment::SyntaxNodeId>,
    slots: &SlotSnapshot,
    visited: &mut BTreeSet<arcweft_lang_syntax::attachment::SyntaxNodeId>,
) -> bool {
    if !visited.insert(attached.id())
        || (slots
            .prepared_source_owner::<ExprId>(attached.id())
            .is_some()
            && !allowed.contains(&attached.id()))
    {
        return false;
    }
    attached.children().iter().all(|child| {
        child.authored_semantic().is_ok_and(|child| {
            child.is_none_or(|child| {
                expression_allocations_are_subset(&child, allowed, slots, visited)
            })
        })
    })
}

fn pattern_tree_is_unallocated(attached: &AttachedPatternNode, slots: &SlotSnapshot) -> bool {
    if slots
        .prepared_source_owner::<PatternId>(attached.id())
        .is_some()
    {
        return false;
    }
    attached.children().is_ok_and(|children| {
        children.iter().all(|child| match child {
            AttachedPatternChild::Pattern { node, .. } => pattern_tree_is_unallocated(node, slots),
            AttachedPatternChild::Type { node, .. } => type_tree_is_unallocated(node, slots),
        })
    })
}

fn child_state_matches(retained: HirSourceChildState, attached: &AttachedSourceExpression) -> bool {
    matches!(
        (retained, attached),
        (
            HirSourceChildState::Authored,
            AttachedSourceExpression::Authored(_)
        ) | (
            HirSourceChildState::Invalid,
            AttachedSourceExpression::Recovered(_)
        ) | (
            HirSourceChildState::Missing,
            AttachedSourceExpression::Missing(_)
        )
    )
}

const fn punctuation(attached: &AttachedSourcePunctuation) -> HirSourcePunctuationState {
    if attached.is_missing() {
        HirSourcePunctuationState::Missing
    } else {
        HirSourcePunctuationState::Present
    }
}

fn expected_item_state(
    attached: &AttachedSourceDeclaration,
    retained: &HirSourceItem,
    item: &HirItem,
    slots: &SlotSnapshot,
    body_issue: Option<HirItemIssue>,
) -> HirItemPoisonState {
    item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| identity_issue(attached, retained))
            .or_else(|| {
                attached
                    .has_missing_type_colon()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .source_type()
                    .has_recovery()
                    .then_some(HirItemIssue::MissingType)
            })
            .or_else(|| {
                slot_is_poisoned(slots, retained.source_type()).then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                matches!(attached.body(), AttachedSourceBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(body_issue)
            .or_else(|| {
                (!matches!(attached.body(), AttachedSourceBody::Missing(_))
                    && !attached.body().is_closed())
                .then_some(HirItemIssue::Recovery)
            }),
    )
}

fn identity_issue(
    attached: &AttachedSourceDeclaration,
    retained: &HirSourceItem,
) -> Option<HirItemIssue> {
    match attached.name() {
        AttachedSourceName::Missing(_) => return Some(HirItemIssue::MissingName),
        AttachedSourceName::Authored { value: Err(_), .. } => {
            return Some(HirItemIssue::MalformedHeader);
        }
        AttachedSourceName::Absent | AttachedSourceName::Authored { .. } => {}
    }
    if retained
        .id()
        .is_some_and(|id| !id.is_canonical_source_family())
    {
        return Some(HirItemIssue::MalformedHeader);
    }
    if retained.id().is_some_and(HirSourceId::has_recovery) {
        return Some(HirItemIssue::Recovery);
    }
    if attached.id().requires_name() && retained.name().is_none() {
        return Some(HirItemIssue::MissingName);
    }
    if retained.id().is_none() && retained.name().is_none() {
        return Some(HirItemIssue::MissingName);
    }
    None
}
