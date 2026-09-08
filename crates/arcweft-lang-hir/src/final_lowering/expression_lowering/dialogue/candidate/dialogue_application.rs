//! Candidate-only E33 lowering nested inside an E34 interpretation.

use arcweft_lang_syntax::attachment::{AttachedCandidateExpressionChild, AttachedCandidateNode};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxAttachedContentApplicationForm,
    SyntaxAttachedContentApplicationProjection, SyntaxBracketTerminator,
    SyntaxDialogueContentProjection,
};

use crate::dialogue_application::{
    HirAttachedContentApplication, HirAttachedContentApplicationFamily, HirDialogueContent,
    HirDialogueContentId, HirDialogueNode, HirDialogueNodeId, HirRawLiteralBody,
};
use crate::expr::{HirCallInvocation, HirExpressionRecoveryIssue, HirRecoveryIssue};
use crate::identity::{ExprId, ScopeId};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::{HirExprSourceRole, expression_component_role};

use super::CandidateCursor;
use crate::final_lowering::StagedHirModuleTransaction;
use crate::final_lowering::expression_lowering::HirAttachedContentApplicationFamilyKind;

impl StagedHirModuleTransaction<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "nested Dialogue application lowering is one closed content/tag/node projection with a single candidate cursor"
    )]
    pub(super) fn lower_nested_candidate_dialogue_application(
        &mut self,
        owner: ExprId,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        projection: &SyntaxAttachedContentApplicationProjection,
        application_family: HirAttachedContentApplicationFamilyKind,
    ) -> Result<(HirAttachedContentApplication, Option<HirRecoveryIssue>), HirLowerFailure> {
        if projection.has_plan() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }

        let expression_projection = node
            .expression_projection()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let mut children = node.semantic_expression_children();
        let target_child = children
            .next()
            .filter(|child| child.component_role() == ExpressionComponentRole::Target)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let (target, invocation, target_recovery) = match target_child {
            AttachedCandidateExpressionChild::Authored { node, .. }
            | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                if matches!(
                    application_family,
                    HirAttachedContentApplicationFamilyKind::ContentCall
                ) {
                    if let Some(ExpressionProjection::Call(call)) = node.expression_projection() {
                        let (invocation, recovery) =
                            self.lower_candidate_call(node, scope, cursor, call)?;
                        let target = invocation
                            .callee()
                            .value_expression()
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                        (target, Some(invocation), recovery)
                    } else {
                        let target = self.lower_candidate_expression_with_family(
                            node,
                            scope,
                            cursor,
                            application_family,
                        )?;
                        (target, Some(HirCallInvocation::bare_value(target)), None)
                    }
                } else {
                    (
                        self.lower_candidate_expression_with_family(
                            node,
                            scope,
                            cursor,
                            application_family,
                        )?,
                        None,
                        None,
                    )
                }
            }
            AttachedCandidateExpressionChild::Missing { .. } => {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
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
        let content_id = HirDialogueContentId::new(owner);
        let content = match projection.content() {
            SyntaxDialogueContentProjection::Missing { .. } => {
                if children.next().is_some() {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                    role: HirExprSourceRole::Content,
                });
                HirDialogueContent::try_new(content_id, Box::new([]), Box::new([]))
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            }
            SyntaxDialogueContentProjection::RawLiteral(literal) => {
                HirDialogueContent::try_new_raw_literal(
                    content_id,
                    HirRawLiteralBody::new(literal.value().into()),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            }
            SyntaxDialogueContentProjection::Present(source) => {
                let mut node_values = vec![None; source.nodes().len()];
                for child in children {
                    let component = child.component_role();
                    let role = expression_component_role(expression_projection, component)
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    let (value, missing) = match child {
                        AttachedCandidateExpressionChild::Authored { node, .. }
                        | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                            (self.lower_candidate_expression(node, scope, cursor)?, false)
                        }
                        AttachedCandidateExpressionChild::Missing { source, .. } => (
                            self.lower_missing_candidate_expression(scope, cursor, role, &source)?,
                            true,
                        ),
                    };
                    if missing {
                        recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
                    } else if self.staged_expression_is_poisoned(value)? {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                            HirExpressionRecoveryIssue::RecoveredChild { role },
                        ));
                    }
                    let destination = match component {
                        ExpressionComponentRole::DialogueNode {
                            ordinal,
                            part: arcweft_lang_syntax::expressions::SyntaxDialogueNodeSourcePart::Interpolation,
                        } => node_values
                            .get_mut(
                                usize::try_from(ordinal)
                                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                            )
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                        ExpressionComponentRole::DialogueNode {
                            ordinal,
                            part: arcweft_lang_syntax::expressions::SyntaxDialogueNodeSourcePart::Expression,
                        } => node_values
                            .get_mut(
                                usize::try_from(ordinal)
                                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                            )
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                        ExpressionComponentRole::DialoguePointAction {
                            ordinal,
                            part: arcweft_lang_syntax::expressions::SyntaxDialoguePointActionSourcePart::Payload,
                        } => node_values
                            .get_mut(
                                usize::try_from(ordinal)
                                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                            )
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
                    };
                    if destination.replace(value).is_some() {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                }

                let mut nodes = Vec::with_capacity(source.nodes().len());
                for (ordinal, source_node) in source.nodes().iter().enumerate() {
                    let id = HirDialogueNodeId::try_new(content_id, ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    nodes.push(HirDialogueNode::new(
                        id,
                        self.project_node(
                            scope,
                            owner,
                            source_node,
                            node_values[ordinal],
                            &mut recovery,
                            id,
                        )?,
                    ));
                }
                let mark_inputs = super::super::project_mark_inputs(content_id, &nodes)?;
                HirDialogueContent::try_new(content_id, nodes.into_boxed_slice(), mark_inputs)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            }
        };

        let family = match application_family {
            HirAttachedContentApplicationFamilyKind::DialogueLine => {
                HirAttachedContentApplicationFamily::DialogueLine {
                    target,
                    plan: None,
                    coordinates: self.dialogue_coordinates(target)?,
                }
            }
            HirAttachedContentApplicationFamilyKind::ContentCall => {
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
}
