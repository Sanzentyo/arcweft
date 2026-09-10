//! Typed lowering for the two retained E34 ambiguity interpretations.
//!
//! Candidate nodes borrow the parser-owned, revision-bound graph. They never
//! receive syntax identities and are never reconstructed from source text.

mod block;
mod call;
mod control;
mod dialogue_application;
mod record;
mod type_ref;

use arcweft_lang_syntax::attachment::{
    AttachedCandidateDialogueOwner, AttachedCandidateExpressionChild, AttachedCandidateGraph,
    AttachedCandidateNode, AttachedCandidatePathExpression, AttachedExpressionNode,
};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxAttachedContentApplicationForm,
    SyntaxDialogueContentProjection, SyntaxDialogueContentRecoveryBoundary, SyntaxExpressionSlot,
    SyntaxPlaceholderKind, SyntaxPostfixBracketProjection, SyntaxSelectedMember,
};

use crate::dialogue_application::{
    HirAttachedContentApplication, HirAttachedContentApplicationFamily, HirDialogueContent,
    HirDialogueContentId, HirDialogueNode, HirDialogueNodeId, HirPostfixBracket,
    HirPostfixBracketCandidates, HirRawLiteralBody,
};
use crate::expr::{
    HirArrayRepeatExpr, HirAwaitExpr, HirBinaryExpr, HirBorrowExpr, HirBracketSequenceExpr,
    HirCallInvocation, HirDereferenceExpr, HirExpr, HirExprError, HirExprKind,
    HirExpressionRecoveryIssue, HirGenericExprIssue, HirIfExpr, HirIndexExpr, HirPipeExpr,
    HirPlaceholderKind, HirPoisonState, HirRangeExpr, HirRecoveryIssue, HirSelectExpr,
    HirSelectedMember, HirTryExpr, HirTupleExpr, HirUnaryExpr, literal_recovery_issue,
};
use crate::identity::{ExprId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole};
use crate::leaf::{HirPathValue, HirShortVariantName};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::{
    HirExprSourceRole, HirInsertionPoint, HirSourceSite, expression_component_role,
};

use super::postfix_failure;
use crate::final_lowering::StagedHirModuleTransaction;
use crate::final_lowering::id_ref_projection::id_ref;
use crate::final_lowering::literal_projection::literal;
use crate::final_lowering::name_projection::{name, name_issue, require_attempted_name_limit};
use crate::final_lowering::path_projection::{
    TypedPathProjection, project_candidate_path, project_type_path,
};

use super::super::HirAttachedContentApplicationFamilyKind;
use super::super::{
    binary_operator, borrow_kind, project_lifetime_path, project_numeric_sequence, unary_operator,
};

struct DialogueCandidateInput<'attached> {
    attached: Option<&'attached AttachedExpressionNode>,
    scope: ScopeId,
    target: ExprId,
    application_family: HirAttachedContentApplicationFamilyKind,
    graph: AttachedCandidateGraph<'attached>,
    root_site: HirSourceSite,
    root_ordinal: u32,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_ambiguous_postfix_candidates(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
        target: ExprId,
        application_family: HirAttachedContentApplicationFamilyKind,
    ) -> Result<HirPostfixBracketCandidates, HirLowerFailure> {
        let index = attached
            .ambiguous_index_candidate()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let dialogue = attached
            .ambiguous_dialogue_candidate()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let root_site = candidate_root_site(self, attached)?;

        let mut index_cursor =
            CandidateCursor::new(owner, SyntheticRole::PostfixIndexCandidateExpression);
        let index = self.lower_index_candidate(
            scope,
            target,
            index,
            root_site.clone(),
            &mut index_cursor,
            0,
        )?;
        let mut dialogue_cursor =
            CandidateCursor::new(owner, SyntheticRole::DialogueContentCandidateExpression);
        let dialogue = self.lower_dialogue_candidate(
            DialogueCandidateInput {
                attached: Some(attached),
                scope,
                target,
                application_family,
                graph: dialogue,
                root_site,
                root_ordinal: 0,
            },
            &mut dialogue_cursor,
        )?;
        Ok(HirPostfixBracketCandidates::Ambiguous { index, dialogue })
    }

    fn lower_index_candidate(
        &mut self,
        scope: ScopeId,
        target: ExprId,
        graph: AttachedCandidateGraph<'_>,
        root_site: HirSourceSite,
        cursor: &mut CandidateCursor,
        root_ordinal: u32,
    ) -> Result<ExprId, HirLowerFailure> {
        let key = SyntheticKey::try_new(
            SyntheticOwner::Expr(cursor.owner),
            cursor.role,
            root_ordinal,
        )
        .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, root_site.clone())?;
        let root = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(root, scope);
        }

        let primary = graph
            .primary()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let index = self.lower_candidate_expression(primary, scope, cursor)?;
        let recovery = if self.staged_expression_is_poisoned(target)? {
            Some(HirRecoveryIssue::InvalidExpression(
                crate::expr::HirExpressionRecoveryIssue::RecoveredChild {
                    role: crate::source_index::HirExprSourceRole::Target,
                },
            ))
        } else if self.staged_expression_is_poisoned(index)? {
            Some(HirRecoveryIssue::InvalidExpression(
                crate::expr::HirExpressionRecoveryIssue::RecoveredChild {
                    role: crate::source_index::HirExprSourceRole::Index,
                },
            ))
        } else {
            None
        };
        let state = recovery.map_or(HirPoisonState::Clean, HirPoisonState::Poisoned);
        let payload = HirExpr::try_new(
            scope,
            HirExprKind::Index(HirIndexExpr::new(target, index)),
            state,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        if payload.is_poisoned() {
            self.stage_candidate_recovery_diagnostic(SyntheticOwner::Expr(root), root_site);
        }
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn lower_dialogue_candidate(
        &mut self,
        input: DialogueCandidateInput<'_>,
        cursor: &mut CandidateCursor,
    ) -> Result<ExprId, HirLowerFailure> {
        let DialogueCandidateInput {
            attached,
            scope,
            target,
            application_family,
            graph,
            root_site,
            root_ordinal,
        } = input;
        let key = SyntheticKey::try_new(
            SyntheticOwner::Expr(cursor.owner),
            cursor.role,
            root_ordinal,
        )
        .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, root_site.clone())?;
        let root = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_content_application(root, scope, application_family);
        }
        let mut recovery = self.staged_expression_is_poisoned(target)?.then_some(
            HirRecoveryIssue::InvalidExpression(HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Target,
            }),
        );
        let body_presence = match graph
            .dialogue_content()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?
        {
            SyntaxDialogueContentProjection::Present(_)
            | SyntaxDialogueContentProjection::RawLiteral(_) => {
                crate::dialogue_application::HirAttachedContentBodyPresence::Present
            }
            SyntaxDialogueContentProjection::Missing { .. } => {
                crate::dialogue_application::HirAttachedContentBodyPresence::Absent
            }
        };
        let content = match graph
            .dialogue_content()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?
        {
            SyntaxDialogueContentProjection::Present(content) => self
                .lower_candidate_dialogue_content(
                    root,
                    scope,
                    graph,
                    content,
                    cursor,
                    &mut recovery,
                )?,
            SyntaxDialogueContentProjection::RawLiteral(literal) => {
                HirDialogueContent::try_new_raw_literal(
                    HirDialogueContentId::new(root),
                    HirRawLiteralBody::new(literal.value().into()),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            }
            SyntaxDialogueContentProjection::Missing { .. } => {
                recovery = Some(HirRecoveryIssue::MissingOperand {
                    role: crate::source_index::HirExprSourceRole::Content,
                });
                HirDialogueContent::try_new(
                    HirDialogueContentId::new(root),
                    Box::new([]),
                    Box::new([]),
                )
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
                HirAttachedContentApplicationFamily::ContentCall {
                    invocation: HirCallInvocation::bare_value(target),
                    evidence: crate::dialogue_application::HirContentCallSemanticEvidence::None,
                }
            }
        };
        let application = HirAttachedContentApplication::try_new_with_body_presence(
            root,
            content,
            family,
            body_presence,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        if let Some(attached) = attached {
            self.source_components.stage_candidate_dialogue_expression(
                self.request.source(),
                cursor.owner,
                root,
                attached,
                graph,
                &application,
            )?;
        }
        let state = recovery.map_or(HirPoisonState::Clean, HirPoisonState::Poisoned);
        let payload = HirExpr::try_new(
            scope,
            HirExprKind::AttachedContentApplication(application),
            state,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        if payload.is_poisoned() {
            self.stage_candidate_recovery_diagnostic(SyntheticOwner::Expr(root), root_site);
        }
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn lower_candidate_dialogue_content(
        &mut self,
        owner: ExprId,
        scope: ScopeId,
        graph: AttachedCandidateGraph<'_>,
        source: &arcweft_lang_syntax::expressions::SyntaxDialogueContent,
        cursor: &mut CandidateCursor,
        recovery: &mut Option<HirRecoveryIssue>,
    ) -> Result<HirDialogueContent, HirLowerFailure> {
        let mut node_values = vec![None; source.nodes().len()];
        for slot in graph
            .dialogue_expression_slots()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?
        {
            let (role, application_family, ordinal) = match slot.owner() {
                AttachedCandidateDialogueOwner::Node { ordinal } => {
                    let node = source
                        .nodes()
                        .get(
                            usize::try_from(ordinal)
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                        )
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    let (part, family) = match node {
                        arcweft_lang_syntax::expressions::SyntaxDialogueNodeProjection::Interpolation(_) => (
                            crate::source_index::HirDialogueNodeSourcePart::Interpolation,
                            HirAttachedContentApplicationFamilyKind::DialogueLine,
                        ),
                        arcweft_lang_syntax::expressions::SyntaxDialogueNodeProjection::ContentApplication(_) => (
                            crate::source_index::HirDialogueNodeSourcePart::Expression,
                            HirAttachedContentApplicationFamilyKind::ContentCall,
                        ),
                        arcweft_lang_syntax::expressions::SyntaxDialogueNodeProjection::PointAction(_) => (
                            crate::source_index::HirDialogueNodeSourcePart::PointAction,
                            HirAttachedContentApplicationFamilyKind::DialogueLine,
                        ),
                        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
                    };
                    (
                        match part {
                            crate::source_index::HirDialogueNodeSourcePart::PointAction => {
                                HirExprSourceRole::DialoguePointAction {
                                    ordinal,
                                    part: crate::source_index::HirDialoguePointActionSourcePart::Payload,
                                }
                            }
                            _ => HirExprSourceRole::DialogueNode { ordinal, part },
                        },
                        family,
                        ordinal,
                    )
                }
            };
            let value = match slot.slot() {
                SyntaxExpressionSlot::Authored => self.lower_candidate_expression_with_family(
                    slot.node(),
                    scope,
                    cursor,
                    application_family,
                )?,
                SyntaxExpressionSlot::Missing => self.lower_missing_candidate_expression(
                    scope,
                    cursor,
                    role,
                    slot.source_span(),
                )?,
            };
            if self.staged_expression_is_poisoned(value)? {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild { role },
                ));
            }
            let destination = node_values
                .get_mut(
                    usize::try_from(ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                )
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            if destination.replace(value).is_some() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }

        let content = HirDialogueContentId::new(owner);
        let mut nodes = Vec::with_capacity(source.nodes().len());
        for (ordinal, source_node) in source.nodes().iter().enumerate() {
            let id = HirDialogueNodeId::try_new(content, ordinal)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            nodes.push(HirDialogueNode::new(
                id,
                self.project_node(
                    scope,
                    owner,
                    source_node,
                    node_values[ordinal],
                    recovery,
                    id,
                )?,
            ));
        }
        let mark_inputs = super::project_mark_inputs(content, &nodes)?;
        HirDialogueContent::try_new(content, nodes.into_boxed_slice(), mark_inputs)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }

    fn lower_nested_ambiguous_postfix_candidates(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        target: ExprId,
        cursor: &mut CandidateCursor,
        application_family: HirAttachedContentApplicationFamilyKind,
    ) -> Result<HirPostfixBracketCandidates, HirLowerFailure> {
        let index = node
            .ambiguous_index_candidate()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let dialogue = node
            .ambiguous_dialogue_candidate()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let root_site = candidate_node_root_site(self, node)?;

        let index_ordinal = cursor.take_expression_ordinal()?;
        let index = self.lower_index_candidate(
            scope,
            target,
            index,
            root_site.clone(),
            cursor,
            index_ordinal,
        )?;
        let dialogue_ordinal = cursor.take_expression_ordinal()?;
        let dialogue = self.lower_dialogue_candidate(
            DialogueCandidateInput {
                attached: None,
                scope,
                target,
                application_family,
                graph: dialogue,
                root_site,
                root_ordinal: dialogue_ordinal,
            },
            cursor,
        )?;
        Ok(HirPostfixBracketCandidates::Ambiguous { index, dialogue })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "this is the exhaustive candidate-node projection into the same closed final expression payload family"
    )]
    fn lower_candidate_expression(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<ExprId, HirLowerFailure> {
        self.lower_candidate_expression_with_family(
            node,
            scope,
            cursor,
            HirAttachedContentApplicationFamilyKind::DialogueLine,
        )
    }

    fn lower_candidate_expression_with_family(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        application_family: HirAttachedContentApplicationFamilyKind,
    ) -> Result<ExprId, HirLowerFailure> {
        let projection = node
            .expression_projection()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let ordinal = cursor.take_expression_ordinal()?;
        let key = SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner), cursor.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let source = HirSourceSite::from_attached_span(
            self.request.source().document(),
            &node.source_span(),
        )
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, source.clone())?;
        let expression = reservation.id();
        if !reservation.is_first_touch() {
            if matches!(
                projection,
                ExpressionProjection::AttachedContentApplication(_)
            ) {
                return self.validate_reused_content_application(
                    expression,
                    scope,
                    application_family,
                );
            }
            return self.validate_reused_expression(expression, scope);
        }

        let (kind, recovery) = match projection {
            ExpressionProjection::Unit => (HirExprKind::Unit, None),
            ExpressionProjection::Literal(value) => {
                let value = literal(value)?;
                let recovery =
                    literal_recovery_issue(&value).map(HirRecoveryIssue::MalformedLiteral);
                (HirExprKind::Literal(value), recovery)
            }
            ExpressionProjection::EntityReference(value) => {
                let value = id_ref(value)?;
                let recovery = value.recovery_issue().map(HirRecoveryIssue::InvalidId);
                (HirExprKind::EntityReference(value), recovery)
            }
            ExpressionProjection::LifetimePath(value) => {
                let value = project_lifetime_path(value)?;
                let recovery = value
                    .recovery()
                    .map(|recovery| HirRecoveryIssue::InvalidLifetimeRegistry(recovery.issue()));
                (HirExprKind::LifetimePath(value), recovery)
            }
            ExpressionProjection::Path => {
                match node
                    .path_expression_view()
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?
                {
                    AttachedCandidatePathExpression::Value(path) => {
                        match project_candidate_path(path)? {
                            TypedPathProjection::Resolved(projected) => {
                                self.record_candidate_path_capture(
                                    expression, scope, path, &projected,
                                )?;
                                (HirExprKind::Path(HirPathValue::Resolved(projected)), None)
                            }
                            TypedPathProjection::Recovered(path) => {
                                let issue = path.issue().clone();
                                (
                                    HirExprKind::Path(HirPathValue::Recovered(path)),
                                    Some(HirRecoveryIssue::InvalidPath(issue)),
                                )
                            }
                        }
                    }
                    AttachedCandidatePathExpression::NominalType(root) => {
                        let path = root
                            .projection()
                            .value()
                            .nominal_path()
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                        (
                            HirExprKind::Path(HirPathValue::Resolved(project_type_path(path)?)),
                            None,
                        )
                    }
                }
            }
            ExpressionProjection::ShortVariant(value) => {
                let value = match value {
                    Ok(value) => HirShortVariantName::Resolved(name(value)?),
                    Err(issue) => {
                        require_attempted_name_limit(issue)?;
                        HirShortVariantName::Recovered(name_issue(issue))
                    }
                };
                let recovery = value.recovery_issue().map(HirRecoveryIssue::InvalidName);
                (HirExprKind::ShortVariant(value), recovery)
            }
            ExpressionProjection::Placeholder(value) => (
                HirExprKind::Placeholder(match value {
                    SyntaxPlaceholderKind::PartialApplication => {
                        HirPlaceholderKind::PartialApplication
                    }
                    SyntaxPlaceholderKind::PipeLeft => HirPlaceholderKind::PipeLeft,
                }),
                None,
            ),
            ExpressionProjection::Tuple(_) => {
                let (elements, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                (HirExprKind::Tuple(HirTupleExpr::new(elements)), recovery)
            }
            ExpressionProjection::BracketSequence(_) => {
                let (elements, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                (
                    HirExprKind::BracketSequence(HirBracketSequenceExpr::new(elements)),
                    recovery,
                )
            }
            ExpressionProjection::NumericBracketSequence(sequence) => {
                let recovery = sequence
                    .has_recovery()
                    .then_some(HirRecoveryIssue::InvalidNumericSequence);
                (
                    HirExprKind::NumericBracketSequence(project_numeric_sequence(sequence)?),
                    recovery,
                )
            }
            ExpressionProjection::ArrayRepeat(_) => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [value, length] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::ArrayRepeat(HirArrayRepeatExpr::new(*value, *length)),
                    recovery,
                )
            }
            ExpressionProjection::Call(call) => {
                let (call, recovery) = self.lower_candidate_call(node, scope, cursor, call)?;
                (HirExprKind::Call(call), recovery)
            }
            ExpressionProjection::Select(member) => {
                let (children, mut recovery) =
                    self.lower_candidate_children(node, scope, cursor)?;
                let [target] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                let member = match member {
                    SyntaxSelectedMember::Name(member) => HirSelectedMember::Name(name(member)?),
                    SyntaxSelectedMember::Missing => {
                        recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                            role: HirExprSourceRole::SelectedMember,
                        });
                        HirSelectedMember::Missing
                    }
                };
                (
                    HirExprKind::Select(HirSelectExpr::new(*target, member)),
                    recovery,
                )
            }
            ExpressionProjection::Index(_) => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [target, index] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Index(HirIndexExpr::new(*target, *index)),
                    recovery,
                )
            }
            ExpressionProjection::AttachedContentApplication(application) => {
                let (application, recovery) = self.lower_nested_candidate_dialogue_application(
                    expression,
                    node,
                    scope,
                    cursor,
                    application,
                    application_family,
                )?;
                (
                    HirExprKind::AttachedContentApplication(application),
                    recovery,
                )
            }
            ExpressionProjection::PostfixBracket(postfix) => {
                let (children, mut recovery) =
                    self.lower_candidate_children(node, scope, cursor)?;
                let [target] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                let candidates = match postfix {
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
                        .lower_nested_ambiguous_postfix_candidates(
                            node,
                            scope,
                            *target,
                            cursor,
                            application_family,
                        )?,
                };
                let postfix = HirPostfixBracket::try_new(*target, candidates)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                postfix
                    .validate_candidate_transaction(cursor.owner, cursor.role, self)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (HirExprKind::PostfixBracket(postfix), recovery)
            }
            ExpressionProjection::Pipe(_) => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [left, right] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (HirExprKind::Pipe(HirPipeExpr::new(*left, *right)), recovery)
            }
            ExpressionProjection::Try { .. } => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (HirExprKind::Try(HirTryExpr::new(*operand)), recovery)
            }
            ExpressionProjection::Await { .. } => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Await(
                        HirAwaitExpr::try_new(*operand, Box::new([]))
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                    ),
                    recovery,
                )
            }
            // Candidate-graph expressions are parser-owned fragments without
            // the specialized attached Thread/Choice relations required by
            // their final semantic owners. Admission is an invariant failure,
            // never a generic Error expression or detached reconstruction.
            ExpressionProjection::Thread(_)
            | ExpressionProjection::Choice
            | ExpressionProjection::Loop => {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            ExpressionProjection::Range {
                start,
                end,
                inclusive,
            } => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let mut children = children.iter().copied();
                let start = if start.is_some() {
                    Some(
                        children
                            .next()
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                    )
                } else {
                    None
                };
                let end = if end.is_some() {
                    Some(
                        children
                            .next()
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                    )
                } else {
                    None
                };
                if children.next().is_some() {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                (
                    HirExprKind::Range(HirRangeExpr::new(start, end, *inclusive)),
                    recovery,
                )
            }
            ExpressionProjection::Record(fields) => {
                let (record, recovery) =
                    self.lower_candidate_record(expression, node, scope, cursor, fields)?;
                (HirExprKind::Record(record), recovery)
            }
            ExpressionProjection::RecordLiteral(fields) => {
                let (record, recovery) =
                    self.lower_candidate_record_literal(expression, node, scope, cursor, fields)?;
                (HirExprKind::RecordLiteral(record), recovery)
            }
            ExpressionProjection::Binary { operator, .. } => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [left, right] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Binary(HirBinaryExpr::new(
                        *left,
                        binary_operator(*operator),
                        *right,
                    )),
                    recovery,
                )
            }
            ExpressionProjection::If { else_branch, .. } => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let (condition, then_branch, else_branch) = match (children.as_ref(), else_branch) {
                    ([condition, then_branch, else_branch], Some(_)) => {
                        (*condition, *then_branch, *else_branch)
                    }
                    ([condition, then_branch], None) => {
                        let source = node
                            .expression_components()
                            .ok_or(HirInvariantFailure::InvalidSourceSpan)?
                            .find(|component| {
                                component.role() == ExpressionComponentRole::ElseBranch
                            })
                            .ok_or(HirInvariantFailure::InvalidSourceSpan)?;
                        let else_branch = self.lower_candidate_implicit_unit(
                            scope,
                            cursor,
                            source.source_span(),
                        )?;
                        (*condition, *then_branch, else_branch)
                    }
                    _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
                };
                (
                    HirExprKind::If(HirIfExpr::new(condition, then_branch, else_branch)),
                    recovery,
                )
            }
            ExpressionProjection::Borrow { kind, .. } => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Borrow(HirBorrowExpr::new(borrow_kind(*kind), *operand)),
                    recovery,
                )
            }
            ExpressionProjection::Dereference { .. } => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Dereference(HirDereferenceExpr::new(*operand)),
                    recovery,
                )
            }
            ExpressionProjection::Unary { operator, .. } => {
                let (children, recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Unary(HirUnaryExpr::new(unary_operator(*operator), *operand)),
                    recovery,
                )
            }
            ExpressionProjection::Closure(_) => {
                let (closure, recovery) =
                    self.lower_candidate_closure(expression, node, scope, cursor)?;
                (HirExprKind::Closure(closure), recovery)
            }
            ExpressionProjection::IfLet { .. } => {
                let (if_let, recovery) =
                    self.lower_candidate_if_let(expression, node, scope, cursor)?;
                (HirExprKind::IfLet(if_let), recovery)
            }
            ExpressionProjection::Match(_) => {
                let (match_expression, recovery) =
                    self.lower_candidate_match(expression, node, scope, cursor)?;
                (HirExprKind::Match(match_expression), recovery)
            }
            ExpressionProjection::Error => {
                let (_, child_recovery) = self.lower_candidate_children(node, scope, cursor)?;
                let issue = HirGenericExprIssue::UnclassifiedSyntax;
                (
                    HirExprKind::Error(HirExprError::new(issue)),
                    child_recovery.or(Some(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::Generic(issue),
                    ))),
                )
            }
            ExpressionProjection::Block => {
                let (block, recovery) =
                    self.lower_candidate_block(expression, node, scope, cursor)?;
                (HirExprKind::Block(block), recovery)
            }
            ExpressionProjection::ComputationBlock(kind) => {
                let (block, recovery) =
                    self.lower_candidate_computation_block(expression, node, scope, cursor, *kind)?;
                (HirExprKind::ComputationBlock(block), recovery)
            }
            ExpressionProjection::NamedBlock(source_name) => {
                let (block, recovery) =
                    self.lower_candidate_named_block(expression, node, scope, cursor, source_name)?;
                (HirExprKind::NamedBlock(block), recovery)
            }
        };
        let state = recovery.map_or(HirPoisonState::Clean, HirPoisonState::Poisoned);
        let payload = HirExpr::try_new(scope, kind, state)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        if payload.is_poisoned() {
            self.stage_candidate_recovery_diagnostic(SyntheticOwner::Expr(expression), source);
        }
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn lower_candidate_children(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<(Box<[ExprId]>, Option<HirRecoveryIssue>), HirLowerFailure> {
        let projection = node
            .expression_projection()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let mut children = Vec::with_capacity(node.semantic_expression_children().len());
        let mut recovery = None;
        for child in node.semantic_expression_children() {
            let role = expression_component_role(projection, child.component_role())
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            let (lowered, missing) = match child {
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
            } else if self.staged_expression_is_poisoned(lowered)? {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild { role },
                ));
            }
            children.push(lowered);
        }
        Ok((children.into_boxed_slice(), recovery))
    }

    fn lower_missing_candidate_expression(
        &mut self,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        role: HirExprSourceRole,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        let ordinal = cursor.take_expression_ordinal()?;
        let key = SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner), cursor.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let site = HirSourceSite::from_attached_span(self.request.source().document(), source)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        if !matches!(site, HirSourceSite::Insertion(_)) {
            return Err(HirInvariantFailure::InvalidSourceSpan.into());
        }
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, site.clone())?;
        let expression = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(expression, scope);
        }
        let payload = HirExpr::try_new(
            scope,
            HirExprKind::Error(HirExprError::new(
                HirGenericExprIssue::TransactionalChildFailure,
            )),
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { role }),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.stage_candidate_recovery_diagnostic(SyntheticOwner::Expr(expression), site);
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn lower_candidate_implicit_unit(
        &mut self,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        let ordinal = cursor.take_expression_ordinal()?;
        let key = SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner), cursor.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let site =
            HirInsertionPoint::try_new(self.request.source().document(), source.range().start())
                .map(HirSourceSite::Insertion)
                .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, site)?;
        let expression = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(expression, scope);
        }
        let payload = HirExpr::try_new(scope, HirExprKind::Unit, HirPoisonState::Clean)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn stage_candidate_recovery_diagnostic(&mut self, owner: SyntheticOwner, site: HirSourceSite) {
        self.stage_recovery_diagnostic(crate::diagnostic::HirRecoveryDiagnostic::new(
            owner,
            crate::diagnostic::HirRecoveryPrimary::owner_whole(owner),
            site,
        ));
    }
}

fn candidate_root_site(
    transaction: &StagedHirModuleTransaction<'_>,
    attached: &AttachedExpressionNode,
) -> Result<HirSourceSite, HirLowerFailure> {
    let content = attached
        .component(ExpressionComponentRole::Content)
        .ok_or(HirInvariantFailure::InvalidSourceSpan)?;
    HirInsertionPoint::try_new(
        transaction.request.source().document(),
        content.range().start(),
    )
    .map(HirSourceSite::Insertion)
    .map_err(|_| HirInvariantFailure::InvalidSourceSpan.into())
}

fn candidate_node_root_site(
    transaction: &StagedHirModuleTransaction<'_>,
    node: AttachedCandidateNode<'_>,
) -> Result<HirSourceSite, HirLowerFailure> {
    let mut components = node
        .expression_components()
        .ok_or(HirInvariantFailure::InvalidSourceSpan)?;
    let content = components.find(|component| component.role() == ExpressionComponentRole::Content);
    let boundary = match content {
        Some(content) => content.source_span().range().start(),
        None => {
            let Some(ExpressionProjection::AttachedContentApplication(application)) =
                node.expression_projection()
            else {
                return Err(HirInvariantFailure::InvalidSourceSpan.into());
            };
            if !matches!(
                application.form(),
                SyntaxAttachedContentApplicationForm::Hash
            ) || !matches!(
                application.content(),
                SyntaxDialogueContentProjection::Missing {
                    boundary: SyntaxDialogueContentRecoveryBoundary::Inline { .. }
                }
            ) {
                return Err(HirInvariantFailure::InvalidSourceSpan.into());
            }
            node.expression_components()
                .ok_or(HirInvariantFailure::InvalidSourceSpan)?
                .find(|component| component.role() == ExpressionComponentRole::Target)
                .map(|component| component.source_span().range().end())
                .ok_or(HirInvariantFailure::InvalidSourceSpan)?
        }
    };
    HirInsertionPoint::try_new(transaction.request.source().document(), boundary)
        .map(HirSourceSite::Insertion)
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan.into())
}

pub(crate) struct CandidateCursor {
    owner: ExprId,
    role: SyntheticRole,
    next_expression: u32,
    next_statement: u32,
    next_type: u32,
    next_pattern: u32,
    next_scope: u32,
    next_local: u32,
}

impl CandidateCursor {
    const fn new(owner: ExprId, role: SyntheticRole) -> Self {
        Self {
            owner,
            role,
            next_expression: 1,
            next_statement: 0,
            next_type: 0,
            next_pattern: 0,
            next_scope: 0,
            next_local: 0,
        }
    }

    pub(crate) const fn owner(&self) -> ExprId {
        self.owner
    }

    pub(crate) const fn role(&self) -> SyntheticRole {
        self.role
    }

    fn take_expression_ordinal(&mut self) -> Result<u32, HirLowerFailure> {
        let ordinal = self.next_expression;
        // Validate the current structural ordinal before advancing. The slot
        // ledger remains the sole aggregate 1,024-descendant accountant.
        SyntheticKey::try_new(SyntheticOwner::Expr(self.owner), self.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        self.next_expression = ordinal
            .checked_add(1)
            .ok_or(HirInvariantFailure::InvalidSlotCommit)?;
        Ok(ordinal)
    }

    pub(crate) fn take_statement_ordinal(&mut self) -> Result<u32, HirLowerFailure> {
        let ordinal = self.next_statement;
        SyntheticKey::try_new(SyntheticOwner::Expr(self.owner), self.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        self.next_statement = ordinal
            .checked_add(1)
            .ok_or(HirInvariantFailure::InvalidSlotCommit)?;
        Ok(ordinal)
    }

    fn take_type_ordinal(&mut self) -> Result<u32, HirLowerFailure> {
        let ordinal = self.next_type;
        SyntheticKey::try_new(SyntheticOwner::Expr(self.owner), self.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        self.next_type = ordinal
            .checked_add(1)
            .ok_or(HirInvariantFailure::InvalidSlotCommit)?;
        Ok(ordinal)
    }

    pub(crate) fn take_pattern_ordinal(&mut self) -> Result<u32, HirLowerFailure> {
        let ordinal = self.next_pattern;
        SyntheticKey::try_new(SyntheticOwner::Expr(self.owner), self.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        self.next_pattern = ordinal
            .checked_add(1)
            .ok_or(HirInvariantFailure::InvalidSlotCommit)?;
        Ok(ordinal)
    }

    pub(crate) fn take_scope_ordinal(&mut self) -> Result<u32, HirLowerFailure> {
        let ordinal = self.next_scope;
        SyntheticKey::try_new(SyntheticOwner::Expr(self.owner), self.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        self.next_scope = ordinal
            .checked_add(1)
            .ok_or(HirInvariantFailure::InvalidSlotCommit)?;
        Ok(ordinal)
    }

    pub(crate) fn take_local_ordinal(&mut self) -> Result<u32, HirLowerFailure> {
        let ordinal = self.next_local;
        SyntheticKey::try_new(SyntheticOwner::Expr(self.owner), self.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        self.next_local = ordinal
            .checked_add(1)
            .ok_or(HirInvariantFailure::InvalidSlotCommit)?;
        Ok(ordinal)
    }
}
