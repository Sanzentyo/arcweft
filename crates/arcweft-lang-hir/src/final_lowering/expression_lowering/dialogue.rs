//! E33 dialogue-content and E34 generic postfix-bracket lowering.
//!
//! The selected E33 path consumes only the attached typed content projection.
//! Candidate-only E34 trees remain a separate attached owner and are never
//! reconstructed from the accepted CST payload or source text.

mod candidate;
mod plan;

pub(crate) use candidate::CandidateCursor;

use arcweft_lang_syntax::attachment::{AttachedExpressionChild, AttachedExpressionNode};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, SyntaxAttachedContentApplicationForm,
    SyntaxAttachedContentApplicationProjection, SyntaxBracketTerminator,
    SyntaxDialogueActionArgumentProjection, SyntaxDialogueContent, SyntaxDialogueContentProjection,
    SyntaxDialogueContentRecoveryBoundary, SyntaxDialogueNodeProjection,
    SyntaxDialogueNodeSourcePart, SyntaxDialoguePointActionIdentity,
    SyntaxDialoguePointActionPayload, SyntaxExpressionSlot, SyntaxPostfixBracketProjection,
    SyntaxPostfixCandidateFailure, SyntaxPostfixCandidateFailureKind,
};
use arcweft_lang_syntax::text::{
    MAX_DIALOGUE_ACTION_ARGUMENTS, MAX_DIALOGUE_ACTION_ARGUMENTS_TOTAL, MAX_DIALOGUE_POINT_ACTIONS,
};

use crate::dialogue_application::{
    HirAttachedContentApplication, HirAttachedContentApplicationFamily,
    HirContentCallNominalDiscriminator, HirContentCallSemanticEvidence, HirDialogueContent,
    HirDialogueContentId, HirDialogueCoordinate, HirDialogueMarkName, HirDialogueNode,
    HirDialogueNodeId, HirDialogueNodeKind, HirDialoguePointAction, HirDialoguePointActionArgument,
    HirDialoguePointActionArgumentId, HirDialoguePointActionIdentity,
    HirDialoguePointActionPayload, HirLinePlan, HirPostfixBracket, HirPostfixBracketCandidates,
    HirPostfixCandidateFailure, HirPostfixCandidateFailureKind, HirRawLiteralBody,
    HirRequiredContentCallNominalDiscriminator, HirRichTextArgumentIssue, HirRichTextIssue,
    HirTextFragment,
};
use crate::expr::{
    HirCallArgument, HirCallCallee, HirCallInvocation, HirExprKind, HirExpressionRecoveryIssue,
    HirGenericExprIssue, HirRecoveryIssue,
};
use crate::final_lowering::id_ref_projection;
use crate::identity::{ExprId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole, TypeId};
use crate::leaf::{HirPathRoot, HirPathSegment, HirPathValue};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::{HirExprSourceRole, expression_component_role};
use crate::type_ref::{HirType, HirTypeKind};

use super::{HirAttachedContentApplicationFamilyKind, StagedHirModuleTransaction};

impl StagedHirModuleTransaction<'_> {
    pub(super) fn preflight_dialogue_content_application(
        attached: &AttachedExpressionNode,
        application: &SyntaxAttachedContentApplicationProjection,
    ) -> Result<(), HirLowerFailure> {
        let targets = attached
            .children()
            .iter()
            .filter(|child| child.component_role() == ExpressionComponentRole::Target)
            .collect::<Vec<_>>();
        if !matches!(targets.as_slice(), [target] if target.authored().is_some()) {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let SyntaxDialogueContentProjection::Present(content) = application.content() else {
            return (attached.children().len() == 1)
                .then_some(())
                .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into());
        };
        let action_count = content
            .nodes()
            .iter()
            .filter(|node| matches!(node, SyntaxDialogueNodeProjection::PointAction(_)))
            .count();
        if action_count > MAX_DIALOGUE_POINT_ACTIONS {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let argument_count = content.nodes().iter().try_fold(
            0usize,
            |count, node| -> Result<usize, HirLowerFailure> {
                let arguments = match node {
                    SyntaxDialogueNodeProjection::PointAction(action) => action.arguments().len(),
                    _ => 0,
                };
                if arguments > MAX_DIALOGUE_ACTION_ARGUMENTS {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                count
                    .checked_add(arguments)
                    .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
            },
        )?;
        if argument_count > MAX_DIALOGUE_ACTION_ARGUMENTS_TOTAL {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        Ok(())
    }

    pub(super) fn preflight_postfix_bracket(
        attached: &AttachedExpressionNode,
        _postfix: &SyntaxPostfixBracketProjection,
    ) -> Result<(), HirLowerFailure> {
        if matches!(
            attached.children(),
            [target]
                if target.component_role() == ExpressionComponentRole::Target
                    && target.authored().is_some()
        ) {
            Ok(())
        } else {
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }

    pub(super) fn lower_dialogue_content_application(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
        projection: &SyntaxAttachedContentApplicationProjection,
        application_family: HirAttachedContentApplicationFamilyKind,
    ) -> Result<(HirAttachedContentApplication, Option<HirRecoveryIssue>), HirLowerFailure> {
        let target_child = Self::expression_child(
            attached,
            ExpressionComponentRole::Target,
            SyntaxExpressionSlot::Authored,
        )?;
        let target_attached = target_child
            .authored_semantic()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let (target, invocation, target_recovery) = match application_family {
            HirAttachedContentApplicationFamilyKind::DialogueLine => (
                self.lower_attached_expression_inner(&target_attached, scope)?,
                None,
                None,
            ),
            HirAttachedContentApplicationFamilyKind::ContentCall => {
                match target_attached.projection() {
                    arcweft_lang_syntax::expressions::ExpressionProjection::Call(call) => {
                        let (invocation, recovery) =
                            self.lower_call_expression(&target_attached, owner, scope, call)?;
                        let target = invocation
                            .callee()
                            .value_expression()
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                        (target, Some(invocation), recovery)
                    }
                    _ => {
                        let target =
                            self.lower_attached_expression_inner(&target_attached, scope)?;
                        (target, Some(HirCallInvocation::bare_value(target)), None)
                    }
                }
            }
        };
        let target_poisoned = self.staged_expression_is_poisoned(target)?;
        let mut recovery = target_recovery.or_else(|| {
            target_poisoned.then_some(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild {
                    role: HirExprSourceRole::Target,
                },
            ))
        });

        if matches!(
            projection.form(),
            SyntaxAttachedContentApplicationForm::Bracket {
                terminator: SyntaxBracketTerminator::RecoveredMissing(_)
            }
        ) {
            recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                role: HirExprSourceRole::CloseBracket,
            });
        }

        let body_presence = match projection.content() {
            SyntaxDialogueContentProjection::Present(_)
            | SyntaxDialogueContentProjection::RawLiteral(_) => {
                crate::dialogue_application::HirAttachedContentBodyPresence::Present
            }
            SyntaxDialogueContentProjection::Missing { .. } => {
                crate::dialogue_application::HirAttachedContentBodyPresence::Absent
            }
        };
        let content = match projection.content() {
            SyntaxDialogueContentProjection::Present(content) => {
                self.lower_dialogue_content(attached, owner, scope, content, &mut recovery)?
            }
            SyntaxDialogueContentProjection::RawLiteral(literal) => {
                HirDialogueContent::try_new_raw_literal(
                    HirDialogueContentId::new(owner),
                    HirRawLiteralBody::new(literal.value().into()),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            }
            SyntaxDialogueContentProjection::Missing { boundary } => {
                let omitted_hash_body = matches!(
                    (application_family, projection.form(), boundary),
                    (
                        HirAttachedContentApplicationFamilyKind::ContentCall,
                        SyntaxAttachedContentApplicationForm::Hash,
                        SyntaxDialogueContentRecoveryBoundary::Inline { .. }
                    )
                );
                if !omitted_hash_body {
                    recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                        role: HirExprSourceRole::Content,
                    });
                }
                HirDialogueContent::try_new(
                    HirDialogueContentId::new(owner),
                    Box::new([]),
                    Box::new([]),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            }
        };
        let family = match application_family {
            HirAttachedContentApplicationFamilyKind::DialogueLine => {
                let coordinates = self.dialogue_coordinates(target)?;
                let plan = attached
                    .dialogue_line_plan()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .map(|plan| self.lower_dialogue_line_plan(&plan, owner, scope, &content))
                    .transpose()?;
                if recovery.is_none() && plan.as_ref().is_some_and(HirLinePlan::has_recovery) {
                    recovery = Some(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild {
                            role: HirExprSourceRole::Plan,
                        },
                    ));
                }
                HirAttachedContentApplicationFamily::DialogueLine {
                    target,
                    plan,
                    coordinates,
                }
            }
            HirAttachedContentApplicationFamilyKind::ContentCall => {
                if attached
                    .dialogue_line_plan()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .is_some()
                {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                let invocation = invocation.ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let (evidence, evidence_recovery) =
                    self.lower_content_call_semantic_evidence(owner, scope, &invocation)?;
                if recovery.is_none() {
                    recovery = evidence_recovery;
                }
                HirAttachedContentApplicationFamily::ContentCall {
                    invocation,
                    evidence,
                }
            }
        };
        let application = HirAttachedContentApplication::try_new_with_body_presence(
            owner,
            content,
            family,
            body_presence,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        if recovery.is_none() {
            application
                .validate_transaction(self)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        }
        Ok((application, recovery))
    }

    /// Retains the one semantic-only fact that cannot be recovered from a
    /// generic content call: an exact `object` nominal discriminator. All
    /// other Content heads are resolved by the presentation catalog later;
    /// HIR does not classify them by source spelling.
    pub(super) fn lower_content_call_semantic_evidence(
        &mut self,
        owner: ExprId,
        scope: ScopeId,
        invocation: &HirCallInvocation,
    ) -> Result<(HirContentCallSemanticEvidence, Option<HirRecoveryIssue>), HirLowerFailure> {
        let HirCallCallee::Value { value: callee } = invocation.callee() else {
            return Ok((HirContentCallSemanticEvidence::None, None));
        };
        let callee_expression = self
            .arenas
            .expressions()
            .resolve_staged(&self.slots, *callee)
            .map_err(HirLowerFailure::from)?;
        let HirExprKind::Path(HirPathValue::Resolved(path)) = callee_expression.kind() else {
            return Ok((HirContentCallSemanticEvidence::None, None));
        };
        if path.root() != HirPathRoot::ImplicitCrate {
            return Ok((HirContentCallSemanticEvidence::None, None));
        }
        let [segment] = path.segments() else {
            return Ok((HirContentCallSemanticEvidence::None, None));
        };
        let is_object = match segment {
            HirPathSegment::Identifier(name) => name.as_str() == "object",
            HirPathSegment::ProjectSymbol(name) => name.as_str() == "object",
        };
        if !is_object {
            return Ok((HirContentCallSemanticEvidence::None, None));
        }

        let mut discriminator = None;
        for (position, argument) in invocation.arguments().iter().enumerate() {
            let HirCallArgument::Named { name, .. } = argument else {
                continue;
            };
            let crate::expr::HirRecoveredName::Valid(name) = name else {
                continue;
            };
            if name.as_str() != "type" {
                continue;
            }
            let crate::expr::HirCallArgument::Named {
                equals: crate::expr::HirRequiredTokenState::Present,
                value: crate::expr::HirCallValue::Present { value },
                ..
            } = argument
            else {
                return Ok(invalid_content_call_semantic_evidence());
            };
            let ordinal = crate::expr::HirCallArgumentOrdinal::try_from_usize(position)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if discriminator.replace((ordinal, *value)).is_some() {
                return Ok(invalid_content_call_semantic_evidence());
            }
        }
        let Some((argument, source)) = discriminator else {
            return Ok(invalid_content_call_semantic_evidence());
        };
        let source_expression = self
            .arenas
            .expressions()
            .resolve_staged(&self.slots, source)
            .map_err(HirLowerFailure::from)?;
        let HirExprKind::Path(HirPathValue::Resolved(path)) = source_expression.kind() else {
            return Ok(invalid_content_call_semantic_evidence());
        };
        if source_expression.is_poisoned() {
            return Ok(invalid_content_call_semantic_evidence());
        }
        let path = path.clone();
        let semantic_only = self.lower_content_call_nominal_type(owner, scope, source, &path)?;
        Ok((
            HirContentCallSemanticEvidence::TextProxyObject {
                nominal_discriminator: HirRequiredContentCallNominalDiscriminator::Present(
                    HirContentCallNominalDiscriminator::new(argument, source, semantic_only),
                ),
            },
            None,
        ))
    }

    fn lower_content_call_nominal_type(
        &mut self,
        owner: ExprId,
        scope: ScopeId,
        source: ExprId,
        path: &crate::leaf::HirPath,
    ) -> Result<TypeId, HirLowerFailure> {
        let key = SyntheticKey::try_new(
            SyntheticOwner::Expr(owner),
            SyntheticRole::ContentCallNominalType,
            0,
        )
        .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let source_site = self.slots.resolve_staged(source)?.source_site().clone();
        let reservation =
            self.arenas
                .types()
                .reserve_synthetic(&mut self.slots, key, source_site)?;
        let type_id = reservation.id();
        if !reservation.is_first_touch() {
            let retained = self
                .arenas
                .types()
                .resolve_staged(&self.slots, type_id)
                .map_err(HirLowerFailure::from)?;
            if retained.scope() != scope || retained.kind() != &HirTypeKind::Path(path.clone()) {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            return Ok(type_id);
        }
        let payload = HirType::try_new(
            type_id,
            HirTypeKind::Path(path.clone()),
            scope,
            crate::expr::HirPoisonState::Clean,
            self,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.arenas
            .types()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    pub(super) fn lower_postfix_bracket(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
        projection: &SyntaxPostfixBracketProjection,
        application_family: HirAttachedContentApplicationFamilyKind,
    ) -> Result<(HirPostfixBracket, Option<HirRecoveryIssue>), HirLowerFailure> {
        let target_child = Self::expression_child(
            attached,
            ExpressionComponentRole::Target,
            SyntaxExpressionSlot::Authored,
        )?;
        let target_attached = target_child
            .authored_semantic()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let target = self.lower_attached_expression_inner(&target_attached, scope)?;
        let mut recovery = self.staged_expression_is_poisoned(target)?.then_some(
            HirRecoveryIssue::InvalidExpression(HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Target,
            }),
        );
        let candidates = match projection {
            SyntaxPostfixBracketProjection::Invalid { index, dialogue } => {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::Generic(
                        HirGenericExprIssue::TransactionalChildFailure,
                    ),
                ));
                HirPostfixBracketCandidates::Invalid {
                    index: postfix_failure(index),
                    dialogue: postfix_failure(dialogue),
                }
            }
            SyntaxPostfixBracketProjection::Ambiguous { .. } => self
                .lower_ambiguous_postfix_candidates(
                    attached,
                    owner,
                    scope,
                    target,
                    application_family,
                )?,
        };
        let postfix = HirPostfixBracket::try_new(target, candidates)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        postfix
            .validate_transaction(owner, self)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((postfix, recovery))
    }

    fn dialogue_coordinates(
        &mut self,
        target: ExprId,
    ) -> Result<Box<[HirDialogueCoordinate]>, HirLowerFailure> {
        let expression = self
            .arenas
            .expressions()
            .resolve_staged(&self.slots, target)?;
        match expression.kind() {
            HirExprKind::Call(call) => {
                HirDialogueCoordinate::from_immediate_arguments(call.arguments())
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
            }
            _ => Ok(Box::new([])),
        }
    }

    fn lower_dialogue_content(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
        source: &SyntaxDialogueContent,
        recovery: &mut Option<HirRecoveryIssue>,
    ) -> Result<HirDialogueContent, HirLowerFailure> {
        let content = HirDialogueContentId::new(owner);
        let nested =
            self.lower_dialogue_nested_expressions(attached, owner, scope, source, recovery)?;
        let mut nodes = Vec::with_capacity(source.nodes().len());
        for (ordinal, source_node) in source.nodes().iter().enumerate() {
            let id = HirDialogueNodeId::try_new(content, ordinal)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let kind = self.project_node(
                scope,
                owner,
                source_node,
                nested.node_values[ordinal],
                recovery,
                id,
            )?;
            nodes.push(HirDialogueNode::new(id, kind));
        }
        let mark_inputs = project_mark_inputs(content, &nodes)?;
        HirDialogueContent::try_new(content, nodes.into_boxed_slice(), mark_inputs)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }

    fn lower_dialogue_nested_expressions(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
        content: &SyntaxDialogueContent,
        recovery: &mut Option<HirRecoveryIssue>,
    ) -> Result<LoweredDialogueNested, HirLowerFailure> {
        let mut node_values = vec![None; content.nodes().len()];
        for child in attached
            .children()
            .iter()
            .filter(|child| child.component_role() != ExpressionComponentRole::Target)
        {
            let component_role = child.component_role();
            let (slot, destination) = match component_role {
                ExpressionComponentRole::DialogueNode {
                    ordinal,
                    part: SyntaxDialogueNodeSourcePart::Interpolation,
                } => {
                    let index = usize::try_from(ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    let SyntaxDialogueNodeProjection::Interpolation(slot) = content
                        .nodes()
                        .get(index)
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?
                    else {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    };
                    (
                        *slot,
                        node_values
                            .get_mut(index)
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                    )
                }
                ExpressionComponentRole::DialogueNode {
                    ordinal,
                    part: SyntaxDialogueNodeSourcePart::Expression,
                } => {
                    let index = usize::try_from(ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    let SyntaxDialogueNodeProjection::ContentApplication(slot) = content
                        .nodes()
                        .get(index)
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?
                    else {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    };
                    (
                        *slot,
                        node_values
                            .get_mut(index)
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                    )
                }
                ExpressionComponentRole::DialoguePointAction {
                    ordinal,
                    part:
                        arcweft_lang_syntax::expressions::SyntaxDialoguePointActionSourcePart::Payload,
                } => {
                    let index = usize::try_from(ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    let SyntaxDialogueNodeProjection::PointAction(action) = content
                        .nodes()
                        .get(index)
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?
                    else {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    };
                    let slot = match action.payload() {
                        SyntaxDialoguePointActionPayload::Call(slot)
                        | SyntaxDialoguePointActionPayload::TimedCue(slot) => slot,
                        SyntaxDialoguePointActionPayload::None => {
                            return Err(HirInvariantFailure::InvalidArenaCommit.into());
                        }
                    };
                    (
                        slot,
                        node_values
                            .get_mut(index)
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                    )
                }
                _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
            };
            if child.authored().is_some() != matches!(slot, SyntaxExpressionSlot::Authored) {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let role = expression_component_role(attached.projection(), component_role)
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            let application_family = match component_role {
                ExpressionComponentRole::DialogueNode {
                    part: SyntaxDialogueNodeSourcePart::Expression,
                    ..
                } => HirAttachedContentApplicationFamilyKind::ContentCall,
                _ => HirAttachedContentApplicationFamilyKind::DialogueLine,
            };
            let value = self.lower_dialogue_nested_expression(
                attached.projection(),
                owner,
                scope,
                child,
                role,
                application_family,
                recovery,
            )?;
            if destination.replace(value).is_some() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }
        Ok(LoweredDialogueNested { node_values })
    }

    fn lower_dialogue_nested_expression(
        &mut self,
        projection: &arcweft_lang_syntax::expressions::ExpressionProjection,
        owner: ExprId,
        scope: ScopeId,
        child: &AttachedExpressionChild,
        role: HirExprSourceRole,
        application_family: HirAttachedContentApplicationFamilyKind,
        recovery: &mut Option<HirRecoveryIssue>,
    ) -> Result<ExprId, HirLowerFailure> {
        let value = if let Some(semantic) = child
            .authored_semantic()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            self.lower_attached_expression_inner_with_family(&semantic, scope, application_family)?
        } else {
            recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
            return self.lower_missing_expression(projection, owner, scope, child);
        };
        if self.staged_expression_is_poisoned(value)? {
            recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild { role },
            ));
        }
        Ok(value)
    }
}

fn invalid_content_call_semantic_evidence()
-> (HirContentCallSemanticEvidence, Option<HirRecoveryIssue>) {
    (
        HirContentCallSemanticEvidence::TextProxyObject {
            nominal_discriminator: HirRequiredContentCallNominalDiscriminator::Invalid,
        },
        Some(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::Generic(HirGenericExprIssue::TransactionalChildFailure),
        )),
    )
}

struct LoweredDialogueNested {
    node_values: Vec<Option<ExprId>>,
}

fn require_absent_expression(expression: Option<ExprId>) -> Result<(), HirLowerFailure> {
    expression
        .is_none()
        .then_some(())
        .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
}

fn project_mark_inputs(
    content: HirDialogueContentId,
    nodes: &[HirDialogueNode],
) -> Result<Box<[(HirDialogueNodeId, HirDialogueMarkName)]>, HirLowerFailure> {
    let mut inputs = Vec::new();
    for node in nodes {
        let HirDialogueNodeKind::PointAction(action) = node.kind() else {
            continue;
        };
        let HirDialoguePointActionIdentity::Mark(name) = action.identity() else {
            continue;
        };
        inputs.push((node.id(), name.clone()));
    }
    let _ = content;
    Ok(inputs.into_boxed_slice())
}

fn project_point_action_argument(
    id: HirDialoguePointActionArgumentId,
    source: &SyntaxDialogueActionArgumentProjection,
    recovery: &mut Option<HirRecoveryIssue>,
) -> HirDialoguePointActionArgument {
    match source {
        SyntaxDialogueActionArgumentProjection::Positional { value } => {
            HirDialoguePointActionArgument::positional(
                id,
                crate::dialogue_application::HirRichTextValue::new(value.decoded().into()),
            )
        }
        SyntaxDialogueActionArgumentProjection::Named { name, value } => match name {
            Ok(name) => HirDialoguePointActionArgument::named(
                id,
                name.as_str().into(),
                crate::dialogue_application::HirRichTextValue::new(value.decoded().into()),
            ),
            Err(_) => {
                let issue = HirRichTextArgumentIssue::InvalidKey;
                recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                    HirRichTextIssue::Argument(issue),
                ));
                HirDialoguePointActionArgument::invalid(id, issue)
            }
        },
        SyntaxDialogueActionArgumentProjection::Invalid { issue, .. } => {
            let issue: HirRichTextArgumentIssue = (*issue).into();
            recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                HirRichTextIssue::Argument(issue),
            ));
            HirDialoguePointActionArgument::invalid(id, issue)
        }
    }
}

impl StagedHirModuleTransaction<'_> {
    fn project_node(
        &mut self,
        scope: ScopeId,
        owner: ExprId,
        source: &SyntaxDialogueNodeProjection,
        expression: Option<ExprId>,
        recovery: &mut Option<HirRecoveryIssue>,
        id: HirDialogueNodeId,
    ) -> Result<HirDialogueNodeKind, HirLowerFailure> {
        match source {
            SyntaxDialogueNodeProjection::Text(value) => {
                require_absent_expression(expression)?;
                Ok(HirDialogueNodeKind::Text(HirTextFragment::new(
                    value.clone(),
                )))
            }
            SyntaxDialogueNodeProjection::Escape(value) => {
                require_absent_expression(expression)?;
                Ok(HirDialogueNodeKind::Escape(*value))
            }
            SyntaxDialogueNodeProjection::Ruby { base, ruby } => {
                require_absent_expression(expression)?;
                let application = self.lower_ruby_surface(scope, owner, id, base, ruby)?;
                Ok(HirDialogueNodeKind::ContentApplication(application))
            }
            SyntaxDialogueNodeProjection::Interpolation(_) => {
                Ok(HirDialogueNodeKind::Interpolation(
                    expression.ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                ))
            }
            SyntaxDialogueNodeProjection::ContentApplication(_) => {
                Ok(HirDialogueNodeKind::ContentApplication(
                    expression.ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                ))
            }
            SyntaxDialogueNodeProjection::PointAction(action) => {
                let identity = match action.identity() {
                    SyntaxDialoguePointActionIdentity::Control(control) => {
                        HirDialoguePointActionIdentity::Control((*control).into())
                    }
                    SyntaxDialoguePointActionIdentity::Host(host) => {
                        HirDialoguePointActionIdentity::Host((*host).into())
                    }
                    SyntaxDialoguePointActionIdentity::Mark(selector) => {
                        if selector.has_recovery() {
                            recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                                HirRichTextIssue::InvalidPayload,
                            ));
                            return Ok(HirDialogueNodeKind::Error(
                                crate::dialogue_application::HirDialogueContentError::InvalidPointAction,
                            ));
                        }
                        let suffix = id_ref_projection::dialogue_mark_suffix(selector.reference())?;
                        HirDialoguePointActionIdentity::Mark(HirDialogueMarkName::new(suffix))
                    }
                    SyntaxDialoguePointActionIdentity::Invalid(_) => {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                            HirRichTextIssue::InvalidPayload,
                        ));
                        return Ok(HirDialogueNodeKind::Error(
                            crate::dialogue_application::HirDialogueContentError::InvalidPointAction,
                        ));
                    }
                };
                let mut arguments = Vec::with_capacity(action.arguments().len());
                for (ordinal, argument) in action.arguments().iter().enumerate() {
                    let argument_id = HirDialoguePointActionArgumentId::try_new(id, ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    arguments.push(project_point_action_argument(
                        argument_id,
                        argument,
                        recovery,
                    ));
                }
                let payload = match (action.payload(), expression) {
                    (SyntaxDialoguePointActionPayload::None, None) => {
                        HirDialoguePointActionPayload::None
                    }
                    (SyntaxDialoguePointActionPayload::Call(_), Some(expression)) => {
                        HirDialoguePointActionPayload::Call(expression)
                    }
                    (SyntaxDialoguePointActionPayload::TimedCue(_), Some(expression)) => {
                        HirDialoguePointActionPayload::TimedCue(expression)
                    }
                    (_, None) => {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                            HirRichTextIssue::InvalidPayload,
                        ));
                        return Ok(HirDialogueNodeKind::Error(
                            crate::dialogue_application::HirDialogueContentError::InvalidPointAction,
                        ));
                    }
                    (SyntaxDialoguePointActionPayload::None, Some(_)) => {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                };
                let action = HirDialoguePointAction::try_new(
                    id,
                    identity,
                    arguments.into_boxed_slice(),
                    payload,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok(HirDialogueNodeKind::PointAction(action))
            }
            SyntaxDialogueNodeProjection::LineBreak(kind) => {
                require_absent_expression(expression)?;
                Ok(HirDialogueNodeKind::LineBreak((*kind).into()))
            }
            SyntaxDialogueNodeProjection::Error(issue) => {
                require_absent_expression(expression)?;
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::Generic(
                        HirGenericExprIssue::TransactionalChildFailure,
                    ),
                ));
                Ok(HirDialogueNodeKind::Error(issue.clone().into()))
            }
        }
    }

    fn lower_ruby_surface(
        &mut self,
        scope: ScopeId,
        owner: ExprId,
        node: HirDialogueNodeId,
        base: &str,
        ruby: &str,
    ) -> Result<ExprId, HirLowerFailure> {
        let recipe = crate::dialogue_application::ruby::HirRubyDesugaring::new(
            owner,
            node.ordinal(),
            scope,
            base,
            ruby,
        );
        let source_site = self.slots.resolve_staged(owner)?.source_site().clone();
        let reservations = recipe
            .keys()?
            .into_iter()
            .map(|key| {
                self.arenas.expressions().reserve_synthetic(
                    &mut self.slots,
                    key,
                    source_site.clone(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ids = std::array::from_fn(|index| reservations[index].id());
        let payloads = recipe.payloads(ids)?;
        for (reservation, payload) in reservations.into_iter().zip(payloads) {
            if reservation.is_first_touch() {
                self.arenas
                    .expressions()
                    .finalize(&mut self.slots, reservation, payload)?;
            } else if self
                .arenas
                .expressions()
                .resolve_staged(&self.slots, reservation.id())?
                != &payload
            {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }
        Ok(ids[2])
    }
}

fn postfix_failure(source: &SyntaxPostfixCandidateFailure) -> HirPostfixCandidateFailure {
    HirPostfixCandidateFailure::new(match source.kind() {
        SyntaxPostfixCandidateFailureKind::EmptyPayload => {
            HirPostfixCandidateFailureKind::EmptyPayload
        }
        SyntaxPostfixCandidateFailureKind::UnexpectedToken => {
            HirPostfixCandidateFailureKind::UnexpectedToken
        }
        SyntaxPostfixCandidateFailureKind::MissingOperand => {
            HirPostfixCandidateFailureKind::MissingOperand
        }
        SyntaxPostfixCandidateFailureKind::TrailingToken => {
            HirPostfixCandidateFailureKind::TrailingToken
        }
        SyntaxPostfixCandidateFailureKind::InvalidDialogueAtom => {
            HirPostfixCandidateFailureKind::InvalidDialogueAtom
        }
    })
}
