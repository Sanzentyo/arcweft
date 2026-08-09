//! Candidate-only E33 lowering nested inside an E34 interpretation.

use arcweft_lang_syntax::attachment::{AttachedCandidateExpressionChild, AttachedCandidateNode};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, SyntaxBracketTerminator, SyntaxDialogueApplicationForm,
    SyntaxDialogueApplicationProjection, SyntaxDialogueContentProjection,
};

use crate::dialogue_application::{
    HirDialogueContent, HirDialogueContentApplication, HirDialogueContentId, HirDialogueNode,
    HirDialogueNodeId, HirRichTextTagId,
};
use crate::expr::{HirExpressionRecoveryIssue, HirRecoveryIssue};
use crate::identity::{ExprId, ScopeId};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::{HirExprSourceRole, expression_component_role};

use super::{CandidateCursor, paired_start_tags, project_node, project_tag};
use crate::final_lowering::StagedHirModuleTransaction;

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
        projection: &SyntaxDialogueApplicationProjection,
    ) -> Result<(HirDialogueContentApplication, Option<HirRecoveryIssue>), HirLowerFailure> {
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
        let target = match target_child {
            AttachedCandidateExpressionChild::Authored { node, .. }
            | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                self.lower_candidate_expression(node, scope, cursor)?
            }
            AttachedCandidateExpressionChild::Missing { .. } => {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        };
        let mut recovery = self.staged_expression_is_poisoned(target)?.then_some(
            HirRecoveryIssue::InvalidExpression(HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Target,
            }),
        );
        if matches!(
            projection.form(),
            SyntaxDialogueApplicationForm::Bracket {
                terminator: SyntaxBracketTerminator::RecoveredMissing(_)
            }
        ) {
            recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                role: HirExprSourceRole::CloseBracket,
            });
        }

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
            SyntaxDialogueContentProjection::Present(source) => {
                let mut node_values = vec![None; source.nodes().len()];
                let mut tag_values = vec![None; source.tags().len()];
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
                        ExpressionComponentRole::RichTextTag {
                            tag,
                            part: arcweft_lang_syntax::expressions::SyntaxRichTextTagSourcePart::Payload,
                        } => tag_values
                            .get_mut(
                                usize::try_from(tag)
                                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                            )
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
                    };
                    if destination.replace(value).is_some() {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                }

                let mut tags = Vec::with_capacity(source.tags().len());
                for (ordinal, source_tag) in source.tags().iter().enumerate() {
                    let id = HirRichTextTagId::try_new(content_id, ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    tags.push(project_tag(
                        self,
                        id,
                        source_tag,
                        tag_values[ordinal],
                        &mut recovery,
                    )?);
                }
                let paired_starts = paired_start_tags(source, &tags)?;
                let mut nodes = Vec::with_capacity(source.nodes().len());
                for (ordinal, source_node) in source.nodes().iter().enumerate() {
                    let id = HirDialogueNodeId::try_new(content_id, ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    nodes.push(HirDialogueNode::new(
                        id,
                        project_node(
                            content_id,
                            &tags,
                            source_node,
                            paired_starts[ordinal],
                            node_values[ordinal],
                            &mut recovery,
                        )?,
                    ));
                }
                HirDialogueContent::try_new(
                    content_id,
                    nodes.into_boxed_slice(),
                    tags.into_boxed_slice(),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            }
        };

        let coordinates = self.dialogue_coordinates(target)?;
        let application =
            HirDialogueContentApplication::try_new(owner, target, content, None, coordinates)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        if recovery.is_none() {
            application
                .validate_transaction(self)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        }
        Ok((application, recovery))
    }
}
