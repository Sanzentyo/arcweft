//! E33 dialogue-content and E34 generic postfix-bracket lowering.
//!
//! The selected E33 path consumes only the attached typed content projection.
//! Candidate-only E34 trees remain a separate attached owner and are never
//! reconstructed from the accepted CST payload or source text.

mod candidate;

pub(crate) use candidate::CandidateCursor;

use arcweft_lang_syntax::attachment::{AttachedExpressionChild, AttachedExpressionNode};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, SyntaxBracketTerminator, SyntaxDialogueApplicationForm,
    SyntaxDialogueApplicationProjection, SyntaxDialogueContent, SyntaxDialogueContentProjection,
    SyntaxDialogueNodeProjection, SyntaxDialogueNodeSourcePart, SyntaxExpressionSlot,
    SyntaxPostfixBracketProjection, SyntaxPostfixCandidateFailure,
    SyntaxPostfixCandidateFailureKind, SyntaxProjectSymbolPath, SyntaxRichTextArgumentProjection,
    SyntaxRichTextEndTagProjection, SyntaxRichTextTagIdentity, SyntaxRichTextTagPayloadProjection,
    SyntaxRichTextTagProjection, SyntaxRichTextTagSourcePart,
};
use arcweft_lang_syntax::name::SyntaxNameIssue;
use arcweft_lang_syntax::text::{
    MAX_RICH_TEXT_CONTENT_ARGUMENTS, MAX_RICH_TEXT_CONTENT_TAGS, MAX_RICH_TEXT_TAG_ARGUMENTS,
};

use crate::dialogue_application::{
    HirDialogueContent, HirDialogueContentApplication, HirDialogueContentId, HirDialogueCoordinate,
    HirDialogueNode, HirDialogueNodeId, HirDialogueNodeKind, HirPostfixBracket,
    HirPostfixBracketCandidates, HirPostfixCandidateFailure, HirPostfixCandidateFailureKind,
    HirRichTextArgument, HirRichTextArgumentId, HirRichTextArgumentIssue, HirRichTextEndTag,
    HirRichTextIssue, HirRichTextTag, HirRichTextTagId, HirRichTextTagIdentity,
    HirRichTextTagPayload, HirRichTextValue, HirRuby, HirTextFragment, HirUnresolvedRichTextTag,
};
use crate::expr::{HirExprKind, HirExpressionRecoveryIssue, HirGenericExprIssue, HirRecoveryIssue};
use crate::identity::{ExprId, ScopeId};
use crate::leaf::HirProjectSymbolSegment;
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::{HirExprSourceRole, expression_component_role};

use super::super::name_projection::{name, require_attempted_name_limit};
use super::StagedHirModuleTransaction;

impl StagedHirModuleTransaction<'_> {
    pub(super) fn preflight_dialogue_content_application(
        attached: &AttachedExpressionNode,
        application: &SyntaxDialogueApplicationProjection,
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
        if content.tags().len() > MAX_RICH_TEXT_CONTENT_TAGS {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let argument_count = content.tags().iter().try_fold(
            0usize,
            |count, tag| -> Result<usize, HirLowerFailure> {
                if tag.arguments().len() > MAX_RICH_TEXT_TAG_ARGUMENTS {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                count
                    .checked_add(tag.arguments().len())
                    .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
            },
        )?;
        if argument_count > MAX_RICH_TEXT_CONTENT_ARGUMENTS {
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
        projection: &SyntaxDialogueApplicationProjection,
    ) -> Result<(HirDialogueContentApplication, Option<HirRecoveryIssue>), HirLowerFailure> {
        if projection.has_plan() {
            // Line-plan lowering is switched with the ordinary Flow owner. It
            // must not be guessed from the old detached plan carrier.
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }

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

        let coordinates = self.dialogue_coordinates(target)?;
        let content = match projection.content() {
            SyntaxDialogueContentProjection::Present(content) => {
                self.lower_dialogue_content(attached, owner, scope, content, &mut recovery)?
            }
            SyntaxDialogueContentProjection::Missing { .. } => {
                recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                    role: HirExprSourceRole::Content,
                });
                HirDialogueContent::try_new(
                    HirDialogueContentId::new(owner),
                    Box::new([]),
                    Box::new([]),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            }
        };
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

    pub(super) fn lower_postfix_bracket(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
        projection: &SyntaxPostfixBracketProjection,
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
            SyntaxPostfixBracketProjection::Ambiguous { .. } => {
                self.lower_ambiguous_postfix_candidates(attached, owner, scope, target)?
            }
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

        let mut tags = Vec::with_capacity(source.tags().len());
        for (ordinal, source_tag) in source.tags().iter().enumerate() {
            let id = HirRichTextTagId::try_new(content, ordinal)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            tags.push(project_tag(
                self,
                id,
                source_tag,
                nested.tag_values[ordinal],
                recovery,
            )?);
        }

        let mut nodes = Vec::with_capacity(source.nodes().len());
        let paired_starts = paired_start_tags(source, &tags)?;
        for (ordinal, source_node) in source.nodes().iter().enumerate() {
            let id = HirDialogueNodeId::try_new(content, ordinal)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let kind = project_node(
                content,
                &tags,
                source_node,
                paired_starts[ordinal],
                nested.node_values[ordinal],
                recovery,
            )?;
            nodes.push(HirDialogueNode::new(id, kind));
        }

        HirDialogueContent::try_new(content, nodes.into_boxed_slice(), tags.into_boxed_slice())
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
        let mut tag_values = vec![None; content.tags().len()];
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
                    (*slot, &mut node_values[index])
                }
                ExpressionComponentRole::RichTextTag {
                    tag,
                    part: SyntaxRichTextTagSourcePart::Payload,
                } => {
                    let index = usize::try_from(tag)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    let tag = content
                        .tags()
                        .get(index)
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    let slot = match tag.payload() {
                        SyntaxRichTextTagPayloadProjection::FxCall(slot)
                        | SyntaxRichTextTagPayloadProjection::DialogueCall(slot)
                        | SyntaxRichTextTagPayloadProjection::Condition(slot) => *slot,
                        SyntaxRichTextTagPayloadProjection::Arguments
                        | SyntaxRichTextTagPayloadProjection::None => {
                            return Err(HirInvariantFailure::InvalidArenaCommit.into());
                        }
                    };
                    (slot, &mut tag_values[index])
                }
                _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
            };
            if child.authored().is_some() != matches!(slot, SyntaxExpressionSlot::Authored) {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let role = expression_component_role(attached.projection(), component_role)
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            let value = self.lower_dialogue_nested_expression(
                attached.projection(),
                owner,
                scope,
                child,
                role,
                recovery,
            )?;
            if destination.replace(value).is_some() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }
        Ok(LoweredDialogueNested {
            node_values,
            tag_values,
        })
    }

    fn lower_dialogue_nested_expression(
        &mut self,
        projection: &arcweft_lang_syntax::expressions::ExpressionProjection,
        owner: ExprId,
        scope: ScopeId,
        child: &AttachedExpressionChild,
        role: HirExprSourceRole,
        recovery: &mut Option<HirRecoveryIssue>,
    ) -> Result<ExprId, HirLowerFailure> {
        let value = if let Some(semantic) = child
            .authored_semantic()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            self.lower_attached_expression_inner(&semantic, scope)?
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

struct LoweredDialogueNested {
    node_values: Vec<Option<ExprId>>,
    tag_values: Vec<Option<ExprId>>,
}

fn paired_start_tags(
    source: &SyntaxDialogueContent,
    tags: &[HirRichTextTag],
) -> Result<Vec<Option<HirRichTextTagId>>, HirLowerFailure> {
    let mut paired_starts = vec![None; source.nodes().len()];
    for (ordinal, source_tag) in source.tags().iter().enumerate() {
        let Some(end_node) = source_tag.paired_end_node() else {
            continue;
        };
        let end_node =
            usize::try_from(end_node).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let paired = tags
            .get(ordinal)
            .map(HirRichTextTag::id)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let destination = paired_starts
            .get_mut(end_node)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        if destination.replace(paired).is_some() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
    }
    Ok(paired_starts)
}

fn project_tag(
    transaction: &mut StagedHirModuleTransaction<'_>,
    id: HirRichTextTagId,
    source: &SyntaxRichTextTagProjection,
    expression: Option<ExprId>,
    recovery: &mut Option<HirRecoveryIssue>,
) -> Result<HirRichTextTag, HirLowerFailure> {
    let identity = project_tag_identity(source.identity(), recovery)?;
    let mut arguments = Vec::with_capacity(source.arguments().len());
    for (ordinal, source_argument) in source.arguments().iter().enumerate() {
        let argument = HirRichTextArgumentId::try_new(id, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        arguments.push(project_argument(argument, source_argument, recovery)?);
    }
    let payload = match source.payload() {
        SyntaxRichTextTagPayloadProjection::Arguments => {
            require_absent_expression(expression)?;
            HirRichTextTagPayload::Arguments
        }
        SyntaxRichTextTagPayloadProjection::None => {
            require_absent_expression(expression)?;
            HirRichTextTagPayload::None
        }
        SyntaxRichTextTagPayloadProjection::FxCall(_)
        | SyntaxRichTextTagPayloadProjection::DialogueCall(_) => {
            let expression = expression.ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            let retained = transaction
                .arenas
                .expressions()
                .resolve_staged(&transaction.slots, expression)?;
            if !matches!(retained.kind(), HirExprKind::Call(_)) {
                recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                    HirRichTextIssue::InvalidPayload,
                ));
            }
            match source.payload() {
                SyntaxRichTextTagPayloadProjection::FxCall(_) => {
                    HirRichTextTagPayload::FxCall(expression)
                }
                SyntaxRichTextTagPayloadProjection::DialogueCall(_) => {
                    HirRichTextTagPayload::DialogueCall(expression)
                }
                _ => unreachable!(),
            }
        }
        SyntaxRichTextTagPayloadProjection::Condition(_) => HirRichTextTagPayload::Condition(
            expression.ok_or(HirInvariantFailure::InvalidArenaCommit)?,
        ),
    };
    HirRichTextTag::try_new(id, identity, arguments.into_boxed_slice(), payload)
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
}

fn require_absent_expression(expression: Option<ExprId>) -> Result<(), HirLowerFailure> {
    expression
        .is_none()
        .then_some(())
        .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
}

fn project_argument(
    id: HirRichTextArgumentId,
    source: &SyntaxRichTextArgumentProjection,
    recovery: &mut Option<HirRecoveryIssue>,
) -> Result<HirRichTextArgument, HirLowerFailure> {
    match source {
        SyntaxRichTextArgumentProjection::Positional { value } => Ok(
            HirRichTextArgument::positional(id, HirRichTextValue::new(value.decoded().into())),
        ),
        SyntaxRichTextArgumentProjection::Named {
            name: Ok(source_name),
            value,
        } => Ok(HirRichTextArgument::named(
            id,
            name(source_name)?,
            HirRichTextValue::new(value.decoded().into()),
        )),
        SyntaxRichTextArgumentProjection::Named {
            name: Err(issue), ..
        } => {
            require_attempted_name_limit(issue)?;
            let issue = HirRichTextArgumentIssue::InvalidKey;
            recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                HirRichTextIssue::Argument(issue),
            ));
            Ok(HirRichTextArgument::invalid(id, issue))
        }
        SyntaxRichTextArgumentProjection::Invalid { issue, .. } => {
            let issue = (*issue).into();
            recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                HirRichTextIssue::Argument(issue),
            ));
            Ok(HirRichTextArgument::invalid(id, issue))
        }
    }
}

fn project_node(
    content: HirDialogueContentId,
    tags: &[HirRichTextTag],
    source: &SyntaxDialogueNodeProjection,
    paired_start: Option<HirRichTextTagId>,
    expression: Option<ExprId>,
    recovery: &mut Option<HirRecoveryIssue>,
) -> Result<HirDialogueNodeKind, HirLowerFailure> {
    match source {
        SyntaxDialogueNodeProjection::Text(value) => {
            require_absent_expression(expression)?;
            Ok(HirDialogueNodeKind::Text(HirTextFragment::new(
                value.clone(),
            )))
        }
        SyntaxDialogueNodeProjection::Raw(value) => {
            require_absent_expression(expression)?;
            Ok(HirDialogueNodeKind::Raw(HirTextFragment::new(
                value.clone(),
            )))
        }
        SyntaxDialogueNodeProjection::Escape(value) => {
            require_absent_expression(expression)?;
            Ok(HirDialogueNodeKind::Escape(*value))
        }
        SyntaxDialogueNodeProjection::Ruby { base, ruby } => {
            require_absent_expression(expression)?;
            Ok(HirDialogueNodeKind::Ruby(HirRuby::new(
                base.clone(),
                ruby.clone(),
            )))
        }
        SyntaxDialogueNodeProjection::AuthoredStartTag { tag }
        | SyntaxDialogueNodeProjection::InferredStartTag { tag } => {
            require_absent_expression(expression)?;
            let tag_index =
                usize::try_from(*tag).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let tag = tags
                .get(tag_index)
                .map(HirRichTextTag::id)
                .filter(|tag| tag.content() == content)
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            if matches!(
                source,
                SyntaxDialogueNodeProjection::AuthoredStartTag { .. }
            ) {
                Ok(HirDialogueNodeKind::AuthoredStartTag(tag))
            } else {
                Ok(HirDialogueNodeKind::InferredStartTag(tag))
            }
        }
        SyntaxDialogueNodeProjection::AuthoredEndTag(end)
        | SyntaxDialogueNodeProjection::InferredEndTag(end) => {
            require_absent_expression(expression)?;
            let projected = project_end_tag(end, paired_start, recovery)?;
            if matches!(source, SyntaxDialogueNodeProjection::AuthoredEndTag(_)) {
                Ok(HirDialogueNodeKind::AuthoredEndTag(projected))
            } else {
                Ok(HirDialogueNodeKind::InferredEndTag(projected))
            }
        }
        SyntaxDialogueNodeProjection::Interpolation(_) => Ok(HirDialogueNodeKind::Interpolation(
            expression.ok_or(HirInvariantFailure::InvalidArenaCommit)?,
        )),
        SyntaxDialogueNodeProjection::LineBreak(kind) => {
            require_absent_expression(expression)?;
            Ok(HirDialogueNodeKind::LineBreak((*kind).into()))
        }
        SyntaxDialogueNodeProjection::Error(issue) => {
            require_absent_expression(expression)?;
            recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::Generic(HirGenericExprIssue::TransactionalChildFailure),
            ));
            Ok(HirDialogueNodeKind::Error(issue.clone().into()))
        }
    }
}

fn project_end_tag(
    source: &SyntaxRichTextEndTagProjection,
    paired_start: Option<HirRichTextTagId>,
    recovery: &mut Option<HirRecoveryIssue>,
) -> Result<HirRichTextEndTag, HirLowerFailure> {
    let identity = source
        .identity()
        .map(|identity| project_tag_identity(identity, recovery))
        .transpose()?;
    let issue: Option<HirRichTextIssue> = source.issue().cloned().map(Into::into);
    if let Some(issue) = issue.as_ref() {
        recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(issue.clone()));
    }
    Ok(HirRichTextEndTag::new(
        paired_start,
        identity,
        source.is_inferred(),
        issue,
    ))
}

fn project_tag_identity(
    source: &SyntaxRichTextTagIdentity,
    recovery: &mut Option<HirRecoveryIssue>,
) -> Result<HirRichTextTagIdentity, HirLowerFailure> {
    match source {
        SyntaxRichTextTagIdentity::Builtin(builtin) => {
            Ok(HirRichTextTagIdentity::Builtin((*builtin).into()))
        }
        SyntaxRichTextTagIdentity::DotSelector(Ok(source_name)) => {
            require_attempted_project_segment_limit(source_name.as_str().len())?;
            let segment = HirProjectSymbolSegment::try_new(source_name.as_str().into())
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            let issue = HirRichTextIssue::UnknownRegisteredTag;
            recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(issue.clone()));
            Ok(HirRichTextTagIdentity::Unresolved(
                HirUnresolvedRichTextTag::new(segment, issue),
            ))
        }
        SyntaxRichTextTagIdentity::DotSelector(Err(issue)) => {
            require_attempted_name_limit(issue)?;
            let segment =
                attempted_project_segment(issue).ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            let issue = HirRichTextIssue::UnknownRegisteredTag;
            recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(issue.clone()));
            Ok(HirRichTextTagIdentity::Unresolved(
                HirUnresolvedRichTextTag::new(segment, issue),
            ))
        }
        SyntaxRichTextTagIdentity::ProjectSymbol(path) => {
            let segment = terminal_project_segment(path)?;
            let issue = HirRichTextIssue::UnknownRegisteredTag;
            recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(issue.clone()));
            Ok(HirRichTextTagIdentity::Unresolved(
                HirUnresolvedRichTextTag::new(segment, issue),
            ))
        }
        SyntaxRichTextTagIdentity::Invalid(issue) => {
            recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(issue.clone().into()));
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }
}

fn terminal_project_segment(
    path: &SyntaxProjectSymbolPath,
) -> Result<HirProjectSymbolSegment, HirLowerFailure> {
    let terminal = path
        .segments()
        .last()
        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
    match terminal {
        Ok(segment) => {
            require_attempted_project_segment_limit(segment.as_str().len())?;
            HirProjectSymbolSegment::try_new(segment.as_str().into())
                .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
        }
        Err(issue) => {
            require_attempted_name_limit(issue)?;
            attempted_project_segment(issue)
                .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
        }
    }
}

fn attempted_project_segment(issue: &SyntaxNameIssue) -> Option<HirProjectSymbolSegment> {
    let spelling = match issue {
        SyntaxNameIssue::Missing => return None,
        SyntaxNameIssue::InvalidStart { spelling }
        | SyntaxNameIssue::InvalidContinuation { spelling } => spelling.as_ref(),
    };
    HirProjectSymbolSegment::try_new(spelling.into())
}

fn require_attempted_project_segment_limit(observed: usize) -> Result<(), HirLowerFailure> {
    if observed <= crate::identity::HirLimit::NameBytes.maximum() {
        Ok(())
    } else {
        Err(crate::lowering::HirLimitError::with_maximum(
            crate::identity::HirLimit::NameBytes,
            observed,
            crate::identity::HirLimit::NameBytes.maximum(),
        )
        .into())
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
