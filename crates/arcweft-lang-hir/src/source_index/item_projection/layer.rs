//! Layer-specific payload re-derivation for final item publication.

use arcweft_lang_syntax::attachment::{
    AttachedLayerDeclaration, AttachedLayerEntry, AttachedLayerExpression, AttachedLayerKind,
    AttachedLayerMember, AttachedLayerPolicy, AttachedLayerReference,
};

use crate::identity::{ExprId, ItemId};
use crate::item::{
    HirAccessibilityPolicy, HirCapturePolicy, HirDeclarationMember, HirDeclarationMemberArena,
    HirDeclarationMemberId, HirDeclarationMemberKind, HirDeclarationMemberPoisonState,
    HirHitTestPolicy, HirInputPolicy, HirItem, HirItemFamily, HirItemIssue, HirItemKind,
    HirLayerAssignmentState, HirLayerExpressionMember, HirLayerKind, HirLayerKindIssue,
    HirLayerMemberPayload, HirLayerMemberValue, HirLayerPolicyMember, HirLayerReferenceMember,
    HirRenderPhase,
};
use crate::leaf::HirIdRefValue;

use super::{
    ItemValidationArenas, expression_owner_matches, item_prefix_matches, item_state, prefix_issue,
    retained_header_item_issue, retained_header_matches, slot_is_poisoned,
};

pub(super) fn payload_matches(
    owner: ItemId,
    attached: &AttachedLayerDeclaration,
    item: &HirItem,
    members: Option<&HirDeclarationMemberArena>,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Layer(layer) = item.kind() else {
        return false;
    };
    if !item_prefix_matches(item, attached.prefix(), slots)
        || !retained_header_matches(layer.header(), attached.header())
        || layer.kind() != layer_kind(attached.kind())
    {
        return false;
    }

    let retained_members = match members {
        Some(members) if members.owner() == owner && members.family() == HirItemFamily::Layer => {
            members.members()
        }
        Some(_) => return false,
        None => &[],
    };
    let mut member_position = 0_usize;
    let mut first_body_issue = None;
    for (entry_position, entry) in attached.body().entries().iter().enumerate() {
        if usize::from(entry.source_ordinal()) != entry_position {
            return false;
        }
        if matches!(entry, AttachedLayerEntry::Recovery { .. }) {
            first_body_issue.get_or_insert(HirItemIssue::InvalidMember);
            continue;
        }
        let Some((id, retained)) = next_member(owner, retained_members, member_position) else {
            return false;
        };
        if retained.id() != id
            || item.members().get(member_position) != Some(&id)
            || !member_matches(retained, entry, item.scope(), slots, arenas)
        {
            return false;
        }
        if entry.has_recovery() || retained.is_poisoned() {
            first_body_issue.get_or_insert(HirItemIssue::InvalidMember);
        }
        member_position += 1;
    }
    if member_position != retained_members.len()
        || member_position != item.members().len()
        || layer.members() != item.members()
    {
        return false;
    }

    let expected_state = item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| retained_header_item_issue(attached.header()))
            .or_else(|| {
                (attached.colon().is_missing() || attached.kind().has_recovery())
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
    );
    item.state() == &expected_state
}

fn member_matches(
    retained: &HirDeclarationMember,
    attached: &AttachedLayerEntry,
    scope: crate::identity::ScopeId,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let payload_matches = match (retained.kind(), attached) {
        (
            HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::Parent(payload)),
            AttachedLayerEntry::Parent(attached),
        )
        | (
            HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::View(payload)),
            AttachedLayerEntry::View(attached),
        )
        | (
            HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::Activity(payload)),
            AttachedLayerEntry::Activity(attached),
        ) => {
            member_metadata_matches(payload, attached)
                && reference_value_matches(payload.value(), attached.value())
        }
        (
            HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Phase(payload)),
            AttachedLayerEntry::Phase(attached),
        ) => {
            member_metadata_matches(payload, attached)
                && phase_value_matches(*payload.value(), attached.value())
        }
        (
            HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Input(payload)),
            AttachedLayerEntry::Input(attached),
        ) => {
            member_metadata_matches(payload, attached)
                && input_value_matches(*payload.value(), attached.value())
        }
        (
            HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::HitTest(payload)),
            AttachedLayerEntry::HitTest(attached),
        ) => {
            member_metadata_matches(payload, attached)
                && hit_test_value_matches(*payload.value(), attached.value())
        }
        (
            HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Capture(payload)),
            AttachedLayerEntry::Capture(attached),
        ) => {
            member_metadata_matches(payload, attached)
                && capture_value_matches(*payload.value(), attached.value())
        }
        (
            HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Accessibility(payload)),
            AttachedLayerEntry::Accessibility(attached),
        ) => {
            member_metadata_matches(payload, attached)
                && accessibility_value_matches(*payload.value(), attached.value())
        }
        (
            HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Z(payload)),
            AttachedLayerEntry::Z(attached),
        )
        | (
            HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Visible(payload)),
            AttachedLayerEntry::Visible(attached),
        )
        | (
            HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Transform(payload)),
            AttachedLayerEntry::Transform(attached),
        ) => {
            member_metadata_matches(payload, attached)
                && expression_value_matches(payload.value(), attached.value(), scope, slots, arenas)
        }
        _ => false,
    };
    payload_matches && retained.state() == payload_state(retained.kind())
}

fn payload_state(kind: &HirDeclarationMemberKind) -> HirDeclarationMemberPoisonState {
    match kind {
        HirDeclarationMemberKind::LayerReference(member) => member.poison_state(),
        HirDeclarationMemberKind::LayerPolicy(member) => match member {
            HirLayerPolicyMember::Phase(payload) => payload.poison_state(),
            HirLayerPolicyMember::Input(payload) => payload.poison_state(),
            HirLayerPolicyMember::HitTest(payload) => payload.poison_state(),
            HirLayerPolicyMember::Capture(payload) => payload.poison_state(),
            HirLayerPolicyMember::Accessibility(payload) => payload.poison_state(),
        },
        HirDeclarationMemberKind::LayerExpression(member) => member.payload().poison_state(),
        _ => unreachable!("called only after matching a Layer member"),
    }
}

fn member_metadata_matches<T, V>(
    retained: &HirLayerMemberPayload<T>,
    attached: &AttachedLayerMember<V>,
) -> bool {
    retained.assignment()
        == if attached.assignment().is_missing() {
            HirLayerAssignmentState::Missing
        } else {
            HirLayerAssignmentState::Present
        }
        && retained.is_duplicate() == attached.state().is_duplicate()
}

fn reference_value_matches(
    retained: &HirLayerMemberValue<HirIdRefValue>,
    attached: &AttachedLayerReference,
) -> bool {
    match (retained, attached) {
        (HirLayerMemberValue::Missing, AttachedLayerReference::Missing(_)) => true,
        (
            HirLayerMemberValue::Present(retained),
            AttachedLayerReference::Retained { reference, .. },
        ) if reference.value().is_ok() => id_ref_matches(retained, reference),
        (
            HirLayerMemberValue::Recovered(Some(retained)),
            AttachedLayerReference::Retained { reference, .. },
        ) if reference.value().is_err() => id_ref_matches(retained, reference),
        (
            HirLayerMemberValue::Recovered(Some(retained)),
            AttachedLayerReference::WrongFamily { reference, .. },
        ) => id_ref_matches(retained, reference),
        _ => false,
    }
}

fn id_ref_matches(
    retained: &HirIdRefValue,
    attached: &arcweft_lang_syntax::id_ref::SyntaxIdRefSyntax,
) -> bool {
    crate::final_lowering::id_ref_projection::id_ref(attached)
        .is_ok_and(|projected| &projected == retained)
}

fn expression_value_matches(
    retained: &HirLayerMemberValue<ExprId>,
    attached: &AttachedLayerExpression,
    scope: crate::identity::ScopeId,
    slots: &crate::slot::SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    match (retained, attached) {
        (HirLayerMemberValue::Missing, AttachedLayerExpression::Missing(_)) => true,
        (HirLayerMemberValue::Present(retained), AttachedLayerExpression::Authored(attached)) => {
            expression_owner_matches(*retained, attached, scope, slots, arenas)
                && !slot_is_poisoned(slots, *retained)
        }
        (
            HirLayerMemberValue::Recovered(Some(retained)),
            AttachedLayerExpression::Authored(attached),
        ) => {
            expression_owner_matches(*retained, attached, scope, slots, arenas)
                && slot_is_poisoned(slots, *retained)
        }
        _ => false,
    }
}

fn phase_value_matches(
    retained: HirLayerMemberValue<HirRenderPhase>,
    attached: &AttachedLayerPolicy,
) -> bool {
    let expected = match attached {
        AttachedLayerPolicy::PhaseBackground(_) => HirRenderPhase::Background,
        AttachedLayerPolicy::PhaseWorld(_) => HirRenderPhase::World,
        AttachedLayerPolicy::PhaseCharacters(_) => HirRenderPhase::Characters,
        AttachedLayerPolicy::PhaseEffects(_) => HirRenderPhase::Effects,
        AttachedLayerPolicy::PhaseDialogue(_) => HirRenderPhase::Dialogue,
        AttachedLayerPolicy::PhaseGameView(_) => HirRenderPhase::GameView,
        AttachedLayerPolicy::PhaseHtmlView(_) => HirRenderPhase::HtmlView,
        AttachedLayerPolicy::PhaseModal(_) => HirRenderPhase::Modal,
        AttachedLayerPolicy::PhaseDebug(_) => HirRenderPhase::Debug,
        AttachedLayerPolicy::PhaseAgentOverlay(_) => HirRenderPhase::AgentOverlay,
        AttachedLayerPolicy::Invalid(_) => {
            return matches!(retained, HirLayerMemberValue::Recovered(None));
        }
        AttachedLayerPolicy::Missing(_) => return matches!(retained, HirLayerMemberValue::Missing),
        _ => return false,
    };
    matches!(retained, HirLayerMemberValue::Present(actual) if actual == expected)
}

fn input_value_matches(
    retained: HirLayerMemberValue<HirInputPolicy>,
    attached: &AttachedLayerPolicy,
) -> bool {
    let expected = match attached {
        AttachedLayerPolicy::InputIgnore(_) => HirInputPolicy::Ignore,
        AttachedLayerPolicy::InputPassThrough(_) => HirInputPolicy::PassThrough,
        AttachedLayerPolicy::InputHitTest(_) => HirInputPolicy::HitTest,
        AttachedLayerPolicy::InputModal(_) => HirInputPolicy::Modal,
        AttachedLayerPolicy::InputCapture(_) => HirInputPolicy::Capture,
        AttachedLayerPolicy::Invalid(_) => {
            return matches!(retained, HirLayerMemberValue::Recovered(None));
        }
        AttachedLayerPolicy::Missing(_) => return matches!(retained, HirLayerMemberValue::Missing),
        _ => return false,
    };
    matches!(retained, HirLayerMemberValue::Present(actual) if actual == expected)
}

fn hit_test_value_matches(
    retained: HirLayerMemberValue<HirHitTestPolicy>,
    attached: &AttachedLayerPolicy,
) -> bool {
    let expected = match attached {
        AttachedLayerPolicy::HitTestNone(_) => HirHitTestPolicy::None,
        AttachedLayerPolicy::HitTestBounds(_) => HirHitTestPolicy::Bounds,
        AttachedLayerPolicy::HitTestViewTree(_) => HirHitTestPolicy::ViewTree,
        AttachedLayerPolicy::HitTestObjectIdMask(_) => HirHitTestPolicy::ObjectIdMask,
        AttachedLayerPolicy::Invalid(_) => {
            return matches!(retained, HirLayerMemberValue::Recovered(None));
        }
        AttachedLayerPolicy::Missing(_) => return matches!(retained, HirLayerMemberValue::Missing),
        _ => return false,
    };
    matches!(retained, HirLayerMemberValue::Present(actual) if actual == expected)
}

fn capture_value_matches(
    retained: HirLayerMemberValue<HirCapturePolicy>,
    attached: &AttachedLayerPolicy,
) -> bool {
    let expected = match attached {
        AttachedLayerPolicy::CaptureNone(_) => HirCapturePolicy::None,
        AttachedLayerPolicy::CaptureColor(_) => HirCapturePolicy::Color,
        AttachedLayerPolicy::CaptureObjectId(_) => HirCapturePolicy::ObjectId,
        AttachedLayerPolicy::CaptureMask(_) => HirCapturePolicy::Mask,
        AttachedLayerPolicy::CaptureAll(_) => HirCapturePolicy::All,
        AttachedLayerPolicy::Invalid(_) => {
            return matches!(retained, HirLayerMemberValue::Recovered(None));
        }
        AttachedLayerPolicy::Missing(_) => return matches!(retained, HirLayerMemberValue::Missing),
        _ => return false,
    };
    matches!(retained, HirLayerMemberValue::Present(actual) if actual == expected)
}

fn accessibility_value_matches(
    retained: HirLayerMemberValue<HirAccessibilityPolicy>,
    attached: &AttachedLayerPolicy,
) -> bool {
    let expected = match attached {
        AttachedLayerPolicy::AccessibilityHidden(_) => HirAccessibilityPolicy::Hidden,
        AttachedLayerPolicy::AccessibilityExposed(_) => HirAccessibilityPolicy::Exposed,
        AttachedLayerPolicy::AccessibilityContainer(_) => HirAccessibilityPolicy::Container,
        AttachedLayerPolicy::Invalid(_) => {
            return matches!(retained, HirLayerMemberValue::Recovered(None));
        }
        AttachedLayerPolicy::Missing(_) => return matches!(retained, HirLayerMemberValue::Missing),
        _ => return false,
    };
    matches!(retained, HirLayerMemberValue::Present(actual) if actual == expected)
}

const fn layer_kind(kind: &AttachedLayerKind) -> HirLayerKind {
    match kind {
        AttachedLayerKind::Background(_) => HirLayerKind::Background,
        AttachedLayerKind::World2d(_) => HirLayerKind::World2d,
        AttachedLayerKind::Character(_) => HirLayerKind::Character,
        AttachedLayerKind::Effects(_) => HirLayerKind::Effects,
        AttachedLayerKind::Dialogue(_) => HirLayerKind::Dialogue,
        AttachedLayerKind::GameView(_) => HirLayerKind::GameView,
        AttachedLayerKind::HtmlView(_) => HirLayerKind::HtmlView,
        AttachedLayerKind::Activity(_) => HirLayerKind::Activity,
        AttachedLayerKind::Modal(_) => HirLayerKind::Modal,
        AttachedLayerKind::Overlay(_) => HirLayerKind::Overlay,
        AttachedLayerKind::Debug(_) => HirLayerKind::Debug,
        AttachedLayerKind::Agent(_) => HirLayerKind::Agent,
        AttachedLayerKind::Offscreen(_) => HirLayerKind::Offscreen,
        AttachedLayerKind::Custom(_) => HirLayerKind::Custom,
        AttachedLayerKind::Missing(_) => HirLayerKind::Recovered(HirLayerKindIssue::Missing),
        AttachedLayerKind::Unknown(_) => HirLayerKind::Recovered(HirLayerKindIssue::Invalid),
    }
}

fn next_member(
    owner: ItemId,
    retained: &[HirDeclarationMember],
    position: usize,
) -> Option<(HirDeclarationMemberId, &HirDeclarationMember)> {
    let ordinal = u32::try_from(position).ok()?;
    Some((
        HirDeclarationMemberId::new(owner, ordinal),
        retained.get(position)?,
    ))
}
