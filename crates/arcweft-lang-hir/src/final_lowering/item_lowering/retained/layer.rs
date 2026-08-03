//! Final Layer lowering from the parser-owned attached declaration schema.

use arcweft_id::RetainedIdentityFamily;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedLayerDeclaration, AttachedLayerEntry, AttachedLayerExpression,
    AttachedLayerKind, AttachedLayerMember, AttachedLayerPolicy, AttachedLayerReference,
};

use crate::identity::{HirLimit, ItemId, ScopeId};
use crate::item::{
    HirAccessibilityPolicy, HirCapturePolicy, HirDeclarationMember, HirDeclarationMemberArena,
    HirDeclarationMemberId, HirDeclarationMemberKind, HirHitTestPolicy, HirInputPolicy, HirItem,
    HirItemFamily, HirItemIssue, HirItemKind, HirLayerAssignmentState, HirLayerDeclaration,
    HirLayerExpressionMember, HirLayerKind, HirLayerKindIssue, HirLayerMemberPayload,
    HirLayerMemberValue, HirLayerPolicyMember, HirLayerReferenceMember, HirRenderPhase,
};
use crate::leaf::HirIdRefValue;
use crate::lower::{HirInvariantFailure, HirLowerFailure};

use super::super::super::{StagedHirModuleTransaction, require_limit};
use super::super::{LoweredItemProjection, item_state};
use super::{project_retained_header, retained_header_issue};

impl StagedHirModuleTransaction<'_> {
    pub(in crate::final_lowering::item_lowering) fn lower_layer_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::LayerDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        preflight_layer_inventory(&attached)?;

        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let header = project_retained_header(attached.header(), RetainedIdentityFamily::Layer)?;
        let kind = project_layer_kind(attached.kind());
        let mut retained_members = Vec::new();
        let mut member_ids = Vec::new();
        let mut first_body_issue = None;

        for (position, entry) in attached.body().entries().iter().enumerate() {
            let expected_ordinal =
                u16::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if entry.source_ordinal() != expected_ordinal {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }

            let Some(member) =
                self.lower_layer_entry(owner, scope, entry, retained_members.len())?
            else {
                first_body_issue.get_or_insert(HirItemIssue::InvalidMember);
                continue;
            };
            let id = member.id();
            if entry.has_recovery() || member.is_poisoned() {
                first_body_issue.get_or_insert(HirItemIssue::InvalidMember);
            }
            retained_members.push(member);
            member_ids.push(id);
        }

        let members = if retained_members.is_empty() {
            None
        } else {
            Some(
                HirDeclarationMemberArena::try_new(
                    owner,
                    HirItemFamily::Layer,
                    retained_members.into_boxed_slice(),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            )
        };
        let issue = prefix
            .issue
            .or_else(|| retained_header_issue(attached.header()))
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
            });
        let declaration = HirLayerDeclaration::try_new(
            owner,
            header,
            kind,
            member_ids.clone().into_boxed_slice(),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Layer(declaration),
            member_ids.into_boxed_slice(),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        Ok(LoweredItemProjection { item, members })
    }

    fn lower_layer_entry(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        entry: &AttachedLayerEntry,
        member_position: usize,
    ) -> Result<Option<HirDeclarationMember>, HirLowerFailure> {
        if matches!(entry, AttachedLayerEntry::Recovery { .. }) {
            return Ok(None);
        }
        let id = next_member_id(owner, member_position)?;
        let kind = match entry {
            AttachedLayerEntry::Parent(member) => {
                HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::Parent(
                    layer_payload(member, lower_reference_value(member.value())?),
                ))
            }
            AttachedLayerEntry::View(member) => {
                HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::View(
                    layer_payload(member, lower_reference_value(member.value())?),
                ))
            }
            AttachedLayerEntry::Activity(member) => {
                HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::Activity(
                    layer_payload(member, lower_reference_value(member.value())?),
                ))
            }
            AttachedLayerEntry::Phase(member) => HirDeclarationMemberKind::LayerPolicy(
                HirLayerPolicyMember::Phase(layer_payload(member, phase_value(member.value())?)),
            ),
            AttachedLayerEntry::Input(member) => HirDeclarationMemberKind::LayerPolicy(
                HirLayerPolicyMember::Input(layer_payload(member, input_value(member.value())?)),
            ),
            AttachedLayerEntry::HitTest(member) => {
                HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::HitTest(layer_payload(
                    member,
                    hit_test_value(member.value())?,
                )))
            }
            AttachedLayerEntry::Capture(member) => {
                HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Capture(layer_payload(
                    member,
                    capture_value(member.value())?,
                )))
            }
            AttachedLayerEntry::Accessibility(member) => {
                HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Accessibility(
                    layer_payload(member, accessibility_value(member.value())?),
                ))
            }
            AttachedLayerEntry::Z(member) => {
                let value = self.lower_layer_expression(member.value(), scope)?;
                HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Z(
                    layer_payload(member, value),
                ))
            }
            AttachedLayerEntry::Visible(member) => {
                let value = self.lower_layer_expression(member.value(), scope)?;
                HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Visible(
                    layer_payload(member, value),
                ))
            }
            AttachedLayerEntry::Transform(member) => {
                let value = self.lower_layer_expression(member.value(), scope)?;
                HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Transform(
                    layer_payload(member, value),
                ))
            }
            AttachedLayerEntry::Recovery { .. } => unreachable!("handled before member allocation"),
        };
        let state = match &kind {
            HirDeclarationMemberKind::LayerReference(member) => member.poison_state(),
            HirDeclarationMemberKind::LayerPolicy(member) => match member {
                HirLayerPolicyMember::Phase(payload) => payload.poison_state(),
                HirLayerPolicyMember::Input(payload) => payload.poison_state(),
                HirLayerPolicyMember::HitTest(payload) => payload.poison_state(),
                HirLayerPolicyMember::Capture(payload) => payload.poison_state(),
                HirLayerPolicyMember::Accessibility(payload) => payload.poison_state(),
            },
            HirDeclarationMemberKind::LayerExpression(member) => member.payload().poison_state(),
            _ => unreachable!("Layer entry constructs only Layer member payloads"),
        };
        HirDeclarationMember::try_new(id, kind, state)
            .map(Some)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }

    fn lower_layer_expression(
        &mut self,
        attached: &AttachedLayerExpression,
        scope: ScopeId,
    ) -> Result<HirLayerMemberValue<crate::identity::ExprId>, HirLowerFailure> {
        match attached {
            AttachedLayerExpression::Missing(_) => Ok(HirLayerMemberValue::Missing),
            AttachedLayerExpression::Authored(expression) => {
                let owner = self.lower_attached_expression(expression, scope)?;
                if self.staged_expression_is_poisoned(owner)? {
                    Ok(HirLayerMemberValue::Recovered(Some(owner)))
                } else {
                    Ok(HirLayerMemberValue::Present(owner))
                }
            }
        }
    }
}

const fn project_layer_kind(kind: &AttachedLayerKind) -> HirLayerKind {
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

fn lower_reference_value(
    attached: &AttachedLayerReference,
) -> Result<HirLayerMemberValue<HirIdRefValue>, HirLowerFailure> {
    let Some(reference) = attached.reference() else {
        return Ok(HirLayerMemberValue::Missing);
    };
    let value = crate::final_lowering::id_ref_projection::id_ref(reference)?;
    if attached.has_recovery() {
        Ok(HirLayerMemberValue::Recovered(Some(value)))
    } else {
        Ok(HirLayerMemberValue::Present(value))
    }
}

fn layer_payload<T, V>(
    member: &AttachedLayerMember<V>,
    value: HirLayerMemberValue<T>,
) -> HirLayerMemberPayload<T> {
    HirLayerMemberPayload::new(
        if member.assignment().is_missing() {
            HirLayerAssignmentState::Missing
        } else {
            HirLayerAssignmentState::Present
        },
        value,
        member.state().is_duplicate(),
    )
}

fn phase_value(
    value: &AttachedLayerPolicy,
) -> Result<HirLayerMemberValue<HirRenderPhase>, HirLowerFailure> {
    let value = match value {
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
        AttachedLayerPolicy::Invalid(_) => return Ok(HirLayerMemberValue::Recovered(None)),
        AttachedLayerPolicy::Missing(_) => return Ok(HirLayerMemberValue::Missing),
        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
    };
    Ok(HirLayerMemberValue::Present(value))
}

fn input_value(
    value: &AttachedLayerPolicy,
) -> Result<HirLayerMemberValue<HirInputPolicy>, HirLowerFailure> {
    let value = match value {
        AttachedLayerPolicy::InputIgnore(_) => HirInputPolicy::Ignore,
        AttachedLayerPolicy::InputPassThrough(_) => HirInputPolicy::PassThrough,
        AttachedLayerPolicy::InputHitTest(_) => HirInputPolicy::HitTest,
        AttachedLayerPolicy::InputModal(_) => HirInputPolicy::Modal,
        AttachedLayerPolicy::InputCapture(_) => HirInputPolicy::Capture,
        AttachedLayerPolicy::Invalid(_) => return Ok(HirLayerMemberValue::Recovered(None)),
        AttachedLayerPolicy::Missing(_) => return Ok(HirLayerMemberValue::Missing),
        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
    };
    Ok(HirLayerMemberValue::Present(value))
}

fn hit_test_value(
    value: &AttachedLayerPolicy,
) -> Result<HirLayerMemberValue<HirHitTestPolicy>, HirLowerFailure> {
    let value = match value {
        AttachedLayerPolicy::HitTestNone(_) => HirHitTestPolicy::None,
        AttachedLayerPolicy::HitTestBounds(_) => HirHitTestPolicy::Bounds,
        AttachedLayerPolicy::HitTestViewTree(_) => HirHitTestPolicy::ViewTree,
        AttachedLayerPolicy::HitTestObjectIdMask(_) => HirHitTestPolicy::ObjectIdMask,
        AttachedLayerPolicy::Invalid(_) => return Ok(HirLayerMemberValue::Recovered(None)),
        AttachedLayerPolicy::Missing(_) => return Ok(HirLayerMemberValue::Missing),
        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
    };
    Ok(HirLayerMemberValue::Present(value))
}

fn capture_value(
    value: &AttachedLayerPolicy,
) -> Result<HirLayerMemberValue<HirCapturePolicy>, HirLowerFailure> {
    let value = match value {
        AttachedLayerPolicy::CaptureNone(_) => HirCapturePolicy::None,
        AttachedLayerPolicy::CaptureColor(_) => HirCapturePolicy::Color,
        AttachedLayerPolicy::CaptureObjectId(_) => HirCapturePolicy::ObjectId,
        AttachedLayerPolicy::CaptureMask(_) => HirCapturePolicy::Mask,
        AttachedLayerPolicy::CaptureAll(_) => HirCapturePolicy::All,
        AttachedLayerPolicy::Invalid(_) => return Ok(HirLayerMemberValue::Recovered(None)),
        AttachedLayerPolicy::Missing(_) => return Ok(HirLayerMemberValue::Missing),
        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
    };
    Ok(HirLayerMemberValue::Present(value))
}

fn accessibility_value(
    value: &AttachedLayerPolicy,
) -> Result<HirLayerMemberValue<HirAccessibilityPolicy>, HirLowerFailure> {
    let value = match value {
        AttachedLayerPolicy::AccessibilityHidden(_) => HirAccessibilityPolicy::Hidden,
        AttachedLayerPolicy::AccessibilityExposed(_) => HirAccessibilityPolicy::Exposed,
        AttachedLayerPolicy::AccessibilityContainer(_) => HirAccessibilityPolicy::Container,
        AttachedLayerPolicy::Invalid(_) => return Ok(HirLayerMemberValue::Recovered(None)),
        AttachedLayerPolicy::Missing(_) => return Ok(HirLayerMemberValue::Missing),
        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
    };
    Ok(HirLayerMemberValue::Present(value))
}

fn next_member_id(
    owner: ItemId,
    position: usize,
) -> Result<HirDeclarationMemberId, HirLowerFailure> {
    let ordinal = u32::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
    Ok(HirDeclarationMemberId::new(owner, ordinal))
}

fn preflight_layer_inventory(attached: &AttachedLayerDeclaration) -> Result<(), HirLowerFailure> {
    require_limit(
        HirLimit::DeclarationMembers,
        attached.body().entries().len(),
    )
}
