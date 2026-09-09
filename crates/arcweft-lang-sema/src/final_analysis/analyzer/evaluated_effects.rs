//! Post-call sealing for runtime-observable expression-statement effects.
//!
//! Preparation retains only callable-owned identity. This pass runs after C1
//! and projects operands exclusively from the final checked applications.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::{
    callable::{
        CallableAttachedContentExecution, CallableAttachedContentParameter,
        CallableEvaluatedEffect, CallableEvaluatedEffectOperandRole, CallableParameterCoordinate,
        CallableParameterPresence, CallableSignatureSchemaDigest, CheckedAttachedContentAdmission,
        CheckedCallApplication, CheckedCallApplicationDigest, CheckedCallArgumentSlotSource,
        CheckedCallAttachedContentOperand, CheckedCallContinuation, CheckedCallOperandDestination,
        CheckedCallResult, CheckedCallRuntimeOperand, CheckedCallSite,
        CheckedLanguageCallableIdentity, DropCallableId, ResolvedCallableStableIdentity,
        ResolvedCallableState,
    },
    env::{StandardDropPolicyCase, StandardDropPolicyValue, StandardEnvironmentValue},
    final_analysis::{
        CheckedContentApplication, CheckedContentApplicationEdges, CheckedDialogueEffectCapture,
        CheckedDialogueEffectPlan, CheckedDialogueEffectSite, CheckedDropFade,
        CheckedDropFadeOperand, CheckedDropInvocation, CheckedDropPolicySource, CheckedEffectField,
        CheckedEvaluatedEffect, CheckedEvaluatedEffectOperand, CheckedEvaluatedEffectOperation,
        CheckedExplicitDropPolicy, CheckedExpression, CheckedExpressionResolution,
        CheckedValueResolution, PreparedContentApplication, PreparedContentEmission,
        PreparedDialogueApplication, PreparedDialogueEffectSite, PreparedEvaluatedEffect,
        PreparedExpressionFact, PreparedStatementPayload,
    },
    semantic_coordinate::{
        SemanticCoordinateIndex, StableCheckedContentFragmentCoordinate,
        StableCheckedValueCoordinate,
    },
};

use crate::callable::CheckedContentRole;
use crate::checked_rich_text::{
    CheckedAttachedContentArgument, CheckedContentApplicationId, CheckedContentApplicationSite,
    CheckedContentEmission, CheckedContentInsertion, CheckedContentValueSource,
    CheckedDialogueContent, CheckedDialogueHostEvent, CheckedDialogueMark, CheckedDialogueToken,
    CheckedRichTextAction, CheckedRichTextReport, PreparedCheckedDialogueMarkCatalog,
    PreparedCheckedDialogueToken, PreparedCheckedRichTextAction, PreparedCheckedRichTextCheck,
    PreparedCheckedRichTextReport,
};
use crate::checked_text_proxy::{
    CheckedTextProxyApplication, CheckedTextProxyApplicationField,
    CheckedTextProxyApplicationMetadata, CheckedTextProxyApplicationValue,
    CheckedTextProxyValueOrigin, PreparedCheckedTextProxyApplication,
    PreparedCheckedTextProxyOrigin, PreparedCheckedTextProxyValue,
};
use arcweft_lang_hir::{
    dialogue_application::HirAttachedContentApplicationFamily,
    expr::HirExprKind,
    identity::{ExprId, LocalId},
    module::HirModule,
};

use super::{Analyzer, FinalSemanticAnalysisError, FinalSemanticAnalysisInput};

#[derive(Default)]
struct EvaluatedEffectOperands {
    message: Option<CheckedEvaluatedEffectOperand>,
    target: Option<CheckedEvaluatedEffectOperand>,
    value: Option<CheckedEvaluatedEffectOperand>,
    event: Option<CheckedEvaluatedEffectOperand>,
    condition: Option<CheckedEvaluatedEffectOperand>,
    policy: Option<CheckedEvaluatedEffectOperand>,
    fields: Vec<CheckedEffectField>,
}

impl EvaluatedEffectOperands {
    fn insert(
        &mut self,
        role: CallableEvaluatedEffectOperandRole,
        operand: CheckedEvaluatedEffectOperand,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let destination = match role {
            CallableEvaluatedEffectOperandRole::Message => &mut self.message,
            CallableEvaluatedEffectOperandRole::Target => &mut self.target,
            CallableEvaluatedEffectOperandRole::Value => &mut self.value,
            CallableEvaluatedEffectOperandRole::Event => &mut self.event,
            CallableEvaluatedEffectOperandRole::Condition => &mut self.condition,
            CallableEvaluatedEffectOperandRole::Policy => &mut self.policy,
        };
        if destination.replace(operand).is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(())
    }

    fn finish(self) -> Result<(), FinalSemanticAnalysisError> {
        if self.message.is_some()
            || self.target.is_some()
            || self.value.is_some()
            || self.event.is_some()
            || self.condition.is_some()
            || self.policy.is_some()
            || !self.fields.is_empty()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedContentOwnerFamily {
    DialogueLine,
    ContentCall,
}

#[derive(Default)]
struct ContentSealTraversal {
    active: BTreeSet<ExprId>,
    visited: BTreeSet<ExprId>,
    ids: BTreeMap<CheckedContentApplicationId, ExprId>,
    fragments: Vec<ContentFragmentFrame>,
    next_fx_ordinal: u32,
}

struct ContentFragmentFrame {
    coordinate: StableCheckedContentFragmentCoordinate,
    next_content_result_ordinal: usize,
}

impl ContentSealTraversal {
    fn enter(&mut self, owner: ExprId) -> Result<(), FinalSemanticAnalysisError> {
        if self.active.contains(&owner) || !self.visited.insert(owner) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        self.active.insert(owner);
        Ok(())
    }

    fn leave(&mut self, owner: ExprId) {
        self.active.remove(&owner);
    }

    fn bind_site(
        &mut self,
        site: &CheckedContentApplicationSite,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if self.ids.insert(site.id().clone(), site.raw()).is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(())
    }

    fn begin_dialogue_root(
        &mut self,
        coordinate: StableCheckedContentFragmentCoordinate,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if !self.active.is_empty() || !self.fragments.is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        self.fragments.push(ContentFragmentFrame {
            coordinate,
            next_content_result_ordinal: 0,
        });
        self.next_fx_ordinal = 0;
        Ok(())
    }

    fn finish_dialogue_root(
        &mut self,
        expected: &StableCheckedContentFragmentCoordinate,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if !self.active.is_empty()
            || self.fragments.len() != 1
            || self
                .fragments
                .last()
                .is_none_or(|frame| &frame.coordinate != expected)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        self.fragments.pop();
        Ok(())
    }

    fn current_fragment(
        &self,
    ) -> Result<StableCheckedContentFragmentCoordinate, FinalSemanticAnalysisError> {
        self.fragments
            .last()
            .map(|frame| frame.coordinate.clone())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
    }

    fn enter_content_result_fragment(
        &mut self,
    ) -> Result<StableCheckedContentFragmentCoordinate, FinalSemanticAnalysisError> {
        let parent = self
            .fragments
            .last_mut()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let ordinal = parent.next_content_result_ordinal;
        parent.next_content_result_ordinal = ordinal
            .checked_add(1)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        let coordinate = parent
            .coordinate
            .try_child(ordinal)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        self.fragments.push(ContentFragmentFrame {
            coordinate: coordinate.clone(),
            next_content_result_ordinal: 0,
        });
        Ok(coordinate)
    }

    fn leave_content_result_fragment(
        &mut self,
        expected: &StableCheckedContentFragmentCoordinate,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if self.fragments.len() <= 1
            || self
                .fragments
                .last()
                .is_none_or(|frame| &frame.coordinate != expected)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        self.fragments.pop();
        Ok(())
    }

    fn next_fx_ordinal(
        &mut self,
    ) -> Result<crate::final_analysis::CheckedFxApplicationOrdinal, FinalSemanticAnalysisError>
    {
        let ordinal = self.next_fx_ordinal;
        self.next_fx_ordinal = self
            .next_fx_ordinal
            .checked_add(1)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        Ok(crate::final_analysis::CheckedFxApplicationOrdinal::from_checked_order(ordinal))
    }
}

fn checked_attached_content_admission(
    body_presence: arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence,
    parameter: Option<CallableAttachedContentParameter>,
    operand: Option<&CheckedCallAttachedContentOperand>,
    content: Option<arcweft_lang_hir::dialogue_application::HirDialogueContentId>,
    surrounding_role: CheckedContentRole,
) -> Result<Option<CheckedAttachedContentAdmission>, FinalSemanticAnalysisError> {
    match (body_presence, parameter, operand, content) {
        (
            arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence::Absent,
            None,
            None,
            None,
        ) => Ok(None),
        (
            arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence::Absent,
            Some(parameter),
            Some(CheckedCallAttachedContentOperand::RuntimeOmitted { .. }),
            None,
        ) if parameter.execution() == CallableAttachedContentExecution::RuntimeContent
            && parameter.presence() != CallableParameterPresence::Required =>
        {
            Ok(None)
        }
        (
            arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence::Present,
            Some(parameter),
            Some(CheckedCallAttachedContentOperand::StructuralPresent { source }),
            Some(content),
        ) if parameter.execution() == CallableAttachedContentExecution::Structural
            && source.raw() == content =>
        {
            Ok(Some(parameter.admission(surrounding_role)))
        }
        (
            arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence::Present,
            Some(parameter),
            Some(CheckedCallAttachedContentOperand::RuntimePresent { source, .. }),
            Some(content),
        ) if parameter.execution() == CallableAttachedContentExecution::RuntimeContent
            && source.raw() == content =>
        {
            Ok(Some(parameter.admission(surrounding_role)))
        }
        _ => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
    }
}

fn checked_content_admits_prepared_token(
    admission: CheckedAttachedContentAdmission,
    token: &PreparedCheckedDialogueToken,
) -> bool {
    match admission {
        CheckedAttachedContentAdmission::Literal => {
            matches!(token, PreparedCheckedDialogueToken::RawLiteral(_))
        }
        CheckedAttachedContentAdmission::Role(CheckedContentRole::Inline) => matches!(
            token,
            PreparedCheckedDialogueToken::Text(_)
                | PreparedCheckedDialogueToken::Escape(_)
                | PreparedCheckedDialogueToken::Interpolation(_)
                | PreparedCheckedDialogueToken::ContentApplication(_)
        ),
        CheckedAttachedContentAdmission::Role(CheckedContentRole::Rich) => match token {
            PreparedCheckedDialogueToken::Text(_)
            | PreparedCheckedDialogueToken::Escape(_)
            | PreparedCheckedDialogueToken::Interpolation(_)
            | PreparedCheckedDialogueToken::ContentApplication(_)
            | PreparedCheckedDialogueToken::LineBreak(
                arcweft_lang_hir::dialogue_application::HirLineBreakKind::Line
                | arcweft_lang_hir::dialogue_application::HirLineBreakKind::Paragraph,
            ) => true,
            PreparedCheckedDialogueToken::PointAction(PreparedCheckedRichTextAction::Control {
                action: crate::checked_rich_text::CheckedDialogueControl::HardBreak,
                ..
            }) => true,
            PreparedCheckedDialogueToken::PointAction(_)
            | PreparedCheckedDialogueToken::LineBreak(
                arcweft_lang_hir::dialogue_application::HirLineBreakKind::Page,
            )
            | PreparedCheckedDialogueToken::RawLiteral(_) => false,
        },
        CheckedAttachedContentAdmission::Role(CheckedContentRole::Dialogue) => {
            !matches!(token, PreparedCheckedDialogueToken::RawLiteral(_))
        }
    }
}

fn seal_proxy_origin(
    origin: PreparedCheckedTextProxyOrigin,
    call_application: CheckedCallApplicationDigest,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<CheckedTextProxyValueOrigin, FinalSemanticAnalysisError> {
    let expression_coordinate = |expression| {
        coordinates
            .expression(expression)
            .map(StableCheckedValueCoordinate::Expression)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)
    };
    match origin {
        PreparedCheckedTextProxyOrigin::AttributeDefault(expression) => {
            expression_coordinate(expression).map(CheckedTextProxyValueOrigin::AttributeDefault)
        }
        PreparedCheckedTextProxyOrigin::Inline { expression, .. } => {
            expression_coordinate(expression).map(|coordinate| {
                CheckedTextProxyValueOrigin::Inline {
                    application: call_application,
                    coordinate,
                }
            })
        }
        PreparedCheckedTextProxyOrigin::CanonicalDefault => {
            Ok(CheckedTextProxyValueOrigin::CanonicalDefault)
        }
        PreparedCheckedTextProxyOrigin::Absent => Ok(CheckedTextProxyValueOrigin::Absent),
    }
}

fn seal_proxy_value<T>(
    value: PreparedCheckedTextProxyValue<T>,
    call_application: CheckedCallApplicationDigest,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<CheckedTextProxyApplicationValue<T>, FinalSemanticAnalysisError> {
    let (value, origin) = value.into_parts();
    Ok(CheckedTextProxyApplicationValue::new(
        value,
        seal_proxy_origin(origin, call_application, coordinates)?,
    ))
}

fn seal_proxy_optional_value<T>(
    value: Option<PreparedCheckedTextProxyValue<T>>,
    call_application: CheckedCallApplicationDigest,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<Option<CheckedTextProxyApplicationValue<T>>, FinalSemanticAnalysisError> {
    value
        .map(|value| seal_proxy_value(value, call_application, coordinates))
        .transpose()
}

fn seal_proxy_application(
    prepared: PreparedCheckedTextProxyApplication,
    definition: &crate::checked_text_proxy::CheckedTextProxyDefinition,
    call_application: CheckedCallApplicationDigest,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<CheckedTextProxyApplication, FinalSemanticAnalysisError> {
    let (definition_id, definition_digest, id, metadata, fields) = prepared.into_parts();
    if definition.id() != &definition_id || definition.digest().ok() != Some(definition_digest) {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    let id = seal_proxy_value(id, call_application, coordinates)?;
    let (role, layer, depth, hit_test) = metadata.into_parts();
    let metadata = CheckedTextProxyApplicationMetadata::new(
        seal_proxy_optional_value(role, call_application, coordinates)?,
        seal_proxy_optional_value(layer, call_application, coordinates)?,
        seal_proxy_optional_value(depth, call_application, coordinates)?,
        seal_proxy_value(hit_test, call_application, coordinates)?,
    );
    let fields = fields
        .into_vec()
        .into_iter()
        .map(|field| {
            let (declaration_ordinal, semantic_id, value, origin) = field.into_parts();
            Ok(CheckedTextProxyApplicationField::new(
                declaration_ordinal,
                semantic_id,
                value,
                seal_proxy_origin(origin, call_application, coordinates)?,
            ))
        })
        .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?;
    CheckedTextProxyApplication::seal(definition, call_application, id, metadata, fields)
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
}

impl Analyzer<'_, '_, '_> {
    /// Prepares one ordinary or RichText-hosted evaluated effect from the
    /// selected callable graph. The authored Pipe remains the structural
    /// owner while its terminal Call owns the final application identity.
    pub(super) fn prepare_evaluated_effect_expression(
        &self,
        module: &HirModule,
        expression: ExprId,
    ) -> Result<Option<PreparedEvaluatedEffect>, FinalSemanticAnalysisError> {
        let authored = module
            .resolve_expr(expression)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let call_owner = match authored.kind() {
            HirExprKind::Call(_) => expression,
            HirExprKind::Pipe(authored_pipe) => match self.facts.expressions().get(&expression) {
                Some(PreparedExpressionFact::Complete(checked)) => {
                    let CheckedExpressionResolution::Pipe(checked_pipe) = checked.resolution()
                    else {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    };
                    if checked_pipe.lookup_left() != authored_pipe.left()
                        || checked_pipe.lookup_right() != authored_pipe.right()
                    {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    checked_pipe.lookup_right()
                }
                Some(PreparedExpressionFact::OwnerBound(prepared)) => {
                    let crate::final_analysis::PreparedOwnerBoundResolution::Pipe(prepared_pipe) =
                        prepared.resolution()
                    else {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    };
                    if prepared_pipe.lookup_left() != authored_pipe.left()
                        || prepared_pipe.lookup_right() != authored_pipe.right()
                    {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    prepared_pipe.lookup_right()
                }
                _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
            },
            _ => return Ok(None),
        };
        let Some(node) = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?
            .selected_nodes()
            .find(|node| node.site() == CheckedCallSite::HirCall(call_owner))
        else {
            return Ok(None);
        };
        let application = node.prefix().application();
        let Some(disposition) = application.selected().schema().evaluated_effect() else {
            return Ok(None);
        };
        if application
            .selected()
            .next_group_for(application.completed_group())
            .is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(Some(PreparedEvaluatedEffect::new(
            expression,
            CheckedCallSite::HirCall(call_owner),
            application.selected().schema().semantic_digest(),
            disposition,
        )))
    }

    pub(super) fn finalize_evaluated_effects(
        &mut self,
        input: &mut FinalSemanticAnalysisInput,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
        structural_edges: &super::super::match_edges::CheckedStructuralEdgeDraft,
        checked_callables: &crate::callable::CheckedCallableCatalog,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut content_traversal = ContentSealTraversal::default();
        let dialogue_applications = self
            .facts
            .expressions()
            .iter()
            .filter_map(|(owner, fact)| match fact {
                PreparedExpressionFact::DialogueApplication(_) => Some(*owner),
                _ => None,
            })
            .collect::<Vec<_>>();
        for owner in dialogue_applications {
            let fragment_coordinate = StableCheckedContentFragmentCoordinate::root(
                StableCheckedValueCoordinate::Expression(
                    coordinates
                        .expression(owner)
                        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
                ),
            );
            content_traversal.begin_dialogue_root(fragment_coordinate.clone())?;
            let taken = self
                .facts
                .take_dialogue_application(owner)
                .map_err(FinalSemanticAnalysisError::from)?;
            let (prepared, content, replacement) = taken.into_parts();
            let sealed = self.seal_dialogue_application(
                *prepared,
                content,
                coordinates,
                structural_edges,
                checked_callables,
                &mut content_traversal,
            )?;
            content_traversal.finish_dialogue_root(&fragment_coordinate)?;
            self.facts
                .publish_sealed_dialogue_application(replacement, sealed)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        }
        // ContentCall rows are children of a DialogueLine rich-text root. A
        // remaining prepared row or catalog entry is an orphan, not an
        // independently sealable expression.
        if self
            .facts
            .expressions()
            .values()
            .any(|fact| matches!(fact, PreparedExpressionFact::ContentApplication(_)))
            || !self.facts.checked_content_is_empty()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let statements = std::mem::take(&mut input.statements)
            .into_iter()
            .map(|(owner, fact)| {
                let fact = match fact {
                    PreparedStatementPayload::EvaluatedEffect(prepared) => {
                        let effect = self.seal_evaluated_effect(prepared)?;
                        PreparedStatementPayload::SealedEvaluatedEffect(Box::new(effect))
                    }
                    fact => fact,
                };
                Ok((owner, fact))
            })
            .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?;
        input.statements = statements;
        Ok(())
    }

    fn seal_dialogue_application(
        &mut self,
        prepared: PreparedDialogueApplication,
        checked_content: PreparedCheckedRichTextCheck,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
        structural_edges: &super::super::match_edges::CheckedStructuralEdgeDraft,
        checked_callables: &crate::callable::CheckedCallableCatalog,
        traversal: &mut ContentSealTraversal,
    ) -> Result<PreparedExpressionFact, FinalSemanticAnalysisError> {
        let (shell, target, application_patch, content, line_result, nested_path) =
            prepared.into_parts();
        if checked_content.content_id() != content {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let (rich_text, markers) = checked_content.into_parts();
        traversal.enter(content.owner())?;
        let fragment_coordinate = traversal.current_fragment()?;
        let rich_text = self.seal_checked_rich_text(
            rich_text,
            markers,
            coordinates,
            structural_edges,
            checked_callables,
            CheckedContentOwnerFamily::DialogueLine,
            CheckedAttachedContentAdmission::Role(CheckedContentRole::Dialogue),
            fragment_coordinate,
            traversal,
        )?;
        traversal.leave(content.owner());
        let (ty, type_selection, effects) = shell
            .into_value_parts()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if ty != crate::types::TypeKind::DialogueLine(Box::new(line_result.clone())) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let resolution = CheckedExpressionResolution::DialogueApplication {
            target,
            application_patch,
            rich_text: Box::new(rich_text),
            line_result,
        };
        let expression = CheckedExpression::value(ty, type_selection, effects, resolution);
        Ok(match nested_path {
            Some(evidence) => {
                PreparedExpressionFact::Complete(expression.with_nested_path_evidence(evidence))
            }
            None => PreparedExpressionFact::Complete(expression),
        })
    }

    fn seal_content_application(
        &mut self,
        prepared: PreparedContentApplication,
        checked_content: Option<PreparedCheckedRichTextCheck>,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
        structural_edges: &super::super::match_edges::CheckedStructuralEdgeDraft,
        checked_callables: &crate::callable::CheckedCallableCatalog,
        surrounding_role: CheckedContentRole,
        traversal: &mut ContentSealTraversal,
    ) -> Result<(PreparedExpressionFact, CheckedContentInsertion), FinalSemanticAnalysisError> {
        let (owner, shell, content, emission) = prepared.into_parts();
        if content.is_some() != checked_content.is_some()
            || content.is_some_and(|content| {
                checked_content
                    .as_ref()
                    .is_none_or(|checked| checked.content_id() != content)
            })
            || !traversal.active.contains(&owner)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let content_result = matches!(&emission, PreparedContentEmission::ContentResult);
        let fragment_coordinate = if content_result {
            traversal.enter_content_result_fragment()?
        } else {
            traversal.current_fragment()?
        };
        let hir_application = {
            let module = self.module(owner.module())?;
            let expression = module
                .resolve_expr(owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            let HirExprKind::AttachedContentApplication(hir_application) = expression.kind() else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            hir_application.clone()
        };
        let HirAttachedContentApplicationFamily::ContentCall { invocation, .. } =
            hir_application.family()
        else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let site = CheckedContentApplicationSite::from_evidence(
            coordinates
                .expression_evidence(owner)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
        );
        traversal.bind_site(&site)?;
        let application_id = site.id().clone();
        let call_application = match invocation.form() {
            arcweft_lang_hir::expr::HirCallInvocationForm::Value => None,
            arcweft_lang_hir::expr::HirCallInvocationForm::Parenthesized => Some(
                self.facts
                    .calls()
                    .get(&owner)
                    .and_then(crate::callable::CallTargetFacts::selected_application)
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            ),
        };
        if call_application.as_ref().is_some_and(|application| {
            application.core().site()
                != (CheckedCallSite::AttachedContentApplication {
                    expression: owner,
                    family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                })
        }) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let parameter = call_application.as_ref().and_then(|application| {
            application
                .core()
                .candidates()
                .selected()
                .schema()
                .attached_content()
        });
        let operand = call_application
            .as_ref()
            .and_then(|application| application.core().execution().attached_content());
        let admission = checked_attached_content_admission(
            hir_application.body_presence(),
            parameter,
            operand,
            content,
            surrounding_role,
        )?;
        let rich_text = match (checked_content, admission) {
            (Some(checked_content), Some(admission)) => {
                let (rich_text, markers) = checked_content.into_parts();
                Some(self.seal_checked_rich_text(
                    rich_text,
                    markers,
                    coordinates,
                    structural_edges,
                    checked_callables,
                    CheckedContentOwnerFamily::ContentCall,
                    admission,
                    fragment_coordinate.clone(),
                    traversal,
                )?)
            }
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        };
        let argument = match rich_text {
            Some(rich_text) => CheckedAttachedContentArgument::present(rich_text),
            None => CheckedAttachedContentArgument::absent(),
        };
        let module = self.module(owner.module())?;
        let expected_dialogue_content = self
            .catalogs
            .world
            .environment()
            .typecheck_env()
            .standard_dialogue_content_type()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let mut content_edges = CheckedContentApplicationEdges::ordinary();
        let insertion = match invocation.form() {
            arcweft_lang_hir::expr::HirCallInvocationForm::Value
                if matches!(&emission, PreparedContentEmission::ContentResult) =>
            {
                if self.facts.calls().get(&owner).is_some()
                    || invocation.arguments().len() != 0
                    || invocation.explicit_type_application().spelling().is_some()
                    || hir_application.body_presence()
                        != arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence::Absent
                    || content.is_some()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let target = invocation
                    .callee()
                    .value_expression()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let Some(PreparedExpressionFact::Complete(checked)) =
                    self.facts.expressions().get(&target)
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                if checked.value_type() != Some(&expected_dialogue_content) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                if shell.value_type() != Some(&expected_dialogue_content) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                CheckedContentInsertion::new(
                    site,
                    fragment_coordinate.clone(),
                    CheckedAttachedContentArgument::absent(),
                    CheckedContentEmission::ContentResult,
                    arcweft_dialogue::InlineFailureSelection::InheritCharacterDialogue,
                )
            }
            arcweft_lang_hir::expr::HirCallInvocationForm::Parenthesized
                if matches!(&emission, PreparedContentEmission::ContentResult) =>
            {
                let application = call_application
                    .as_ref()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                if application.core().site()
                    != (CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                    })
                    || application.result().value_type() != shell.value_type()
                    || application.result().value_type() != Some(&expected_dialogue_content)
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                CheckedContentInsertion::new(
                    site,
                    fragment_coordinate.clone(),
                    argument,
                    CheckedContentEmission::ContentResult,
                    arcweft_dialogue::InlineFailureSelection::InheritCharacterDialogue,
                )
            }
            arcweft_lang_hir::expr::HirCallInvocationForm::Parenthesized
                if matches!(&emission, PreparedContentEmission::ObjectSpan(_)) =>
            {
                if content.is_none()
                    || !matches!(
                        shell.result(),
                        super::super::prepared::PreparedExpressionResult::NonValue(
                            super::super::prepared::PreparedNonValueExpressionResult::ContentEmission(
                                crate::callable::ContentCallableIdentity::TextProxyObject { .. },
                            )
                        )
                    )
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let application = call_application
                    .as_ref()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                if application.core().site()
                    != (CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                    })
                    || !matches!(
                        application.result().content_emission(),
                        Some(crate::callable::ContentCallableIdentity::TextProxyObject { .. })
                    )
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let selected = application.core().candidates().selected();
                if !matches!(
                    selected.id(),
                    crate::callable::CallableCandidateId::Content(
                        crate::callable::ContentCallableIdentity::TextProxyObject { .. }
                    )
                ) || !matches!(
                    selected.schema().validator(),
                    crate::callable::CallableValidator::Content(
                        crate::callable::ContentCallableIdentity::TextProxyObject { .. }
                    )
                ) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let schema = selected.schema();
                let attached = schema
                    .attached_content()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                if attached
                    != crate::callable::CallableAttachedContentParameter::text_proxy_object()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let PreparedContentEmission::ObjectSpan(prepared_proxy) = emission else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let definition = self
                    .text_proxies
                    .as_ref()
                    .and_then(|catalog| catalog.get(prepared_proxy.definition_id()))
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                    .checked();
                let proxy = seal_proxy_application(
                    prepared_proxy,
                    definition,
                    application.digest(),
                    coordinates,
                )?;
                CheckedContentInsertion::new(
                    site,
                    fragment_coordinate.clone(),
                    argument,
                    CheckedContentEmission::ObjectSpan(proxy),
                    arcweft_dialogue::InlineFailureSelection::InheritCharacterDialogue,
                )
            }
            arcweft_lang_hir::expr::HirCallInvocationForm::Parenthesized
                if matches!(&emission, PreparedContentEmission::LanguageCallable(_)) =>
            {
                let super::super::prepared::PreparedExpressionResult::NonValue(
                    super::super::prepared::PreparedNonValueExpressionResult::ContentEmission(
                        callable,
                    ),
                ) = shell.result()
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let application = call_application
                    .as_ref()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                if application.core().site()
                    != (CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                    })
                    || application.result().content_emission() != Some(*callable)
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let selected = application.core().candidates().selected();
                let crate::callable::CallableCandidateId::Content(
                    crate::callable::ContentCallableIdentity::Language { definition, schema },
                ) = selected.id()
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let row = arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG
                    .get(*definition)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let identity =
                    crate::callable::ContentCallableIdentity::language(*definition, *schema);
                if row.schema_digest() != *schema || identity != *callable {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                selected
                    .schema()
                    .attached_content()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let checked_emission = match emission {
                    PreparedContentEmission::LanguageCallable(identity) => {
                        if identity != *callable {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        }
                        match row.emission_family() {
                        arcweft_presentation::rich_text::PresentationContentEmissionFamily::Strong
                        | arcweft_presentation::rich_text::PresentationContentEmissionFamily::Em
                        | arcweft_presentation::rich_text::PresentationContentEmissionFamily::Color
                        | arcweft_presentation::rich_text::PresentationContentEmissionFamily::Font
                        | arcweft_presentation::rich_text::PresentationContentEmissionFamily::Size
                        | arcweft_presentation::rich_text::PresentationContentEmissionFamily::Style
                        | arcweft_presentation::rich_text::PresentationContentEmissionFamily::Layout
                        | arcweft_presentation::rich_text::PresentationContentEmissionFamily::Transform => {
                            let parameters = self
                                .content_parameter_values(module, &application, *definition)
                                .map_err(|_| {
                                    FinalSemanticAnalysisError::WrongPayloadFamily
                                })?;
                            CheckedContentEmission::Modifier(
                                crate::checked_rich_text::CheckedContentModifier::new(
                                    *definition,
                                    *schema,
                                    parameters,
                                ),
                            )
                        }
                        arcweft_presentation::rich_text::PresentationContentEmissionFamily::Fx => {
                            if *definition
                                != arcweft_presentation::rich_text::PresentationContentCallableDefinitionId::Fx
                            {
                                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                            }
                            let ordinal = traversal.next_fx_ordinal()?;
                            let fx_application = self
                                .checked_content_fx_application(module, &application, ordinal)
                                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            let payload_plan = fx_application
                                .seal_content_fx(application, &hir_application, coordinates)
                                .map_err(|source| FinalSemanticAnalysisError::FxEdgePlan {
                                    owner,
                                    source,
                                })?;
                            content_edges = CheckedContentApplicationEdges::fx(payload_plan);
                            CheckedContentEmission::Fx(fx_application)
                        }
                        arcweft_presentation::rich_text::PresentationContentEmissionFamily::Ruby => {
                            if *definition
                                != arcweft_presentation::rich_text::PresentationContentCallableDefinitionId::Ruby
                            {
                                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                            }
                            let parameters = self
                                .content_parameter_values(module, &application, *definition)
                                .map_err(|_| {
                                    FinalSemanticAnalysisError::WrongPayloadFamily
                                })?;
                            let reading = parameters
                                .into_iter()
                                .find(|parameter| {
                                    parameter.id()
                                        == arcweft_presentation::rich_text::PresentationContentCallableParameterId::Value
                                })
                                .and_then(|parameter| match parameter.value() {
                                    crate::final_analysis::CheckedCompileTimeValue::Scalar(
                                        crate::checked_compile_time::CheckedCompileTimeScalar::Text(
                                            value,
                                        ),
                                    ) => Some(value.clone()),
                                    _ => None,
                                })
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            CheckedContentEmission::Ruby(
                                crate::checked_rich_text::CheckedContentRuby::new(reading),
                            )
                        }
                        arcweft_presentation::rich_text::PresentationContentEmissionFamily::Raw => {
                            if *definition
                                != arcweft_presentation::rich_text::PresentationContentCallableDefinitionId::Raw
                            {
                                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                            }
                            let body = hir_application
                                .content()
                                .raw_literal()
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            CheckedContentEmission::Raw(
                                crate::checked_rich_text::CheckedRawLiteral::new(body.as_str()),
                            )
                        }
                        }
                    }
                    PreparedContentEmission::ContentResult
                    | PreparedContentEmission::ObjectSpan(_) => {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                };
                CheckedContentInsertion::new(
                    site,
                    fragment_coordinate.clone(),
                    argument,
                    checked_emission,
                    arcweft_dialogue::InlineFailureSelection::InheritCharacterDialogue,
                )
            }
            _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
        };
        let effects = shell.effects().clone();
        let result = shell.result().clone();
        let invocation_form = invocation.form();
        let content_application = if content_result
            && invocation_form == arcweft_lang_hir::expr::HirCallInvocationForm::Value
        {
            let target = invocation
                .callee()
                .value_expression()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let source = CheckedContentValueSource::from_evidence(
                coordinates
                    .expression_evidence(target)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
            );
            CheckedContentApplication::value(application_id, source)
        } else if content_result
            && invocation_form == arcweft_lang_hir::expr::HirCallInvocationForm::Parenthesized
        {
            let application = call_application
                .as_ref()
                .map(crate::callable::CheckedCallApplication::digest)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            CheckedContentApplication::content_result_call(application_id, application)
        } else {
            let application = call_application
                .as_ref()
                .map(crate::callable::CheckedCallApplication::digest)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            CheckedContentApplication::emission_call(application_id, application, content_edges)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?
        };
        let resolution =
            CheckedExpressionResolution::ContentApplication(Box::new(content_application));
        let expression = match result {
            super::super::prepared::PreparedExpressionResult::Value(value) => {
                PreparedExpressionFact::Complete(CheckedExpression::value(
                    value.ty().clone(),
                    value.type_selection(),
                    effects,
                    resolution,
                ))
            }
            super::super::prepared::PreparedExpressionResult::NonValue(
                super::super::prepared::PreparedNonValueExpressionResult::ContentEmission(callable),
            ) => PreparedExpressionFact::Complete(CheckedExpression::content_emission(
                callable, effects, resolution,
            )),
        };
        if content_result {
            traversal.leave_content_result_fragment(&fragment_coordinate)?;
        }
        Ok((expression, insertion))
    }

    fn seal_checked_rich_text(
        &mut self,
        prepared: PreparedCheckedRichTextReport,
        mut markers: PreparedCheckedDialogueMarkCatalog,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
        structural_edges: &super::super::match_edges::CheckedStructuralEdgeDraft,
        checked_callables: &crate::callable::CheckedCallableCatalog,
        expected_family: CheckedContentOwnerFamily,
        admission: CheckedAttachedContentAdmission,
        fragment_coordinate: StableCheckedContentFragmentCoordinate,
        traversal: &mut ContentSealTraversal,
    ) -> Result<CheckedRichTextReport, FinalSemanticAnalysisError> {
        let (content, diagnostics, effect_plan) = prepared.into_parts();
        let (content_id, tokens, diagnostics_complete) = content.into_parts();
        if !diagnostics_complete || !diagnostics.is_empty() || markers.content() != content_id {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if tokens
            .iter()
            .any(|token| !checked_content_admits_prepared_token(admission, token))
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let expected_effect_roots = tokens
            .iter()
            .filter_map(|token| match token {
                PreparedCheckedDialogueToken::PointAction(
                    PreparedCheckedRichTextAction::Host {
                        action:
                            CheckedDialogueHostEvent::TimedCue { call, .. }
                            | CheckedDialogueHostEvent::Call { call },
                        ..
                    },
                ) => Some(*call),
                _ => None,
            })
            .collect::<Vec<_>>();
        let prepared_effect_sites = effect_plan.into_parts();
        if prepared_effect_sites.len() != expected_effect_roots.len()
            || prepared_effect_sites
                .iter()
                .zip(&expected_effect_roots)
                .enumerate()
                .any(|(index, (site, root))| {
                    site.root() != *root
                        || site.id().get() != u32::try_from(index).unwrap_or(u32::MAX)
                })
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let mut marker_count = 0_usize;
        let tokens = tokens
            .into_vec()
            .into_iter()
            .map(|token| {
                Ok(match token {
                    PreparedCheckedDialogueToken::Text(text) => CheckedDialogueToken::Text(text),
                    PreparedCheckedDialogueToken::Escape(value) => {
                        CheckedDialogueToken::Escape(value)
                    }
                    PreparedCheckedDialogueToken::PointAction(action) => {
                        let action = match action {
                            PreparedCheckedRichTextAction::Control { action, fields } => {
                                CheckedRichTextAction::Control { action, fields }
                            }
                            PreparedCheckedRichTextAction::Host {
                                owner,
                                action,
                                fields,
                            } => CheckedRichTextAction::Host {
                                owner,
                                action,
                                fields,
                            },
                            PreparedCheckedRichTextAction::Marker { mark } => {
                                let mark = markers
                                    .take(mark)
                                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                                let (hir, diagnostic_name) = mark.into_parts();
                                let expected = u32::try_from(marker_count)
                                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                                if hir.content() != content_id || hir.ordinal().get() != expected {
                                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                                }
                                marker_count = marker_count
                                    .checked_add(1)
                                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
                                let coordinate = coordinates
                                    .dialogue_mark(self.executable, hir)
                                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                                CheckedRichTextAction::Marker(CheckedDialogueMark::new(
                                    coordinate,
                                    diagnostic_name,
                                ))
                            }
                        };
                        CheckedDialogueToken::PointAction(action)
                    }
                    PreparedCheckedDialogueToken::Interpolation(expression) => {
                        CheckedDialogueToken::Interpolation(expression)
                    }
                    PreparedCheckedDialogueToken::ContentApplication(reference) => self
                        .seal_nested_content_application(
                            content_id,
                            reference,
                            coordinates,
                            structural_edges,
                            checked_callables,
                            admission
                                .role()
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                            traversal,
                        )?,
                    PreparedCheckedDialogueToken::LineBreak(kind) => {
                        CheckedDialogueToken::LineBreak(kind)
                    }
                    PreparedCheckedDialogueToken::RawLiteral(body) => {
                        CheckedDialogueToken::RawLiteral(body)
                    }
                })
            })
            .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?;
        let module = self.module(content_id.owner().module())?;
        let expression = module
            .resolve_expr(content_id.owner())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let actual_family = match application.family() {
            arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
                ..
            } => CheckedContentOwnerFamily::DialogueLine,
            arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::ContentCall {
                ..
            } => CheckedContentOwnerFamily::ContentCall,
        };
        if actual_family != expected_family {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if application.content().id() != content_id
            || application.content().marks().len() != marker_count
            || !markers.is_empty()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let effect_sites = prepared_effect_sites
            .into_vec()
            .into_iter()
            .map(|site| {
                self.seal_dialogue_effect_site(
                    site,
                    coordinates,
                    structural_edges,
                    checked_callables,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let effect_plan = CheckedDialogueEffectPlan::new(effect_sites);
        Ok(CheckedRichTextReport::new(
            admission,
            fragment_coordinate,
            CheckedDialogueContent::new(content_id, tokens, true),
            diagnostics.into_vec(),
        )
        .with_effect_plan(effect_plan))
    }

    fn seal_nested_content_application(
        &mut self,
        parent: arcweft_lang_hir::dialogue_application::HirDialogueContentId,
        reference: crate::checked_rich_text::PreparedContentApplicationRef,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
        structural_edges: &super::super::match_edges::CheckedStructuralEdgeDraft,
        checked_callables: &crate::callable::CheckedCallableCatalog,
        surrounding_role: CheckedContentRole,
        traversal: &mut ContentSealTraversal,
    ) -> Result<CheckedDialogueToken, FinalSemanticAnalysisError> {
        if reference.node().content() != parent
            || reference.expression().module() != parent.owner().module()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let parent_module = self.module(parent.owner().module())?;
        let parent_expression = parent_module
            .resolve_expr(parent.owner())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::AttachedContentApplication(parent_application) = parent_expression.kind()
        else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let Some(node) = parent_application.content().nodes().get(
            usize::try_from(reference.node().ordinal())
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
        ) else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if node.id() != reference.node()
            || !matches!(
                node.kind(),
                arcweft_lang_hir::dialogue_application::HirDialogueNodeKind::ContentApplication(
                    expression
                ) if *expression == reference.expression()
            )
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        traversal.enter(reference.expression())?;
        let taken = self
            .facts
            .take_content_application(reference.expression())
            .map_err(FinalSemanticAnalysisError::from)?;
        let (prepared, content, replacement) = taken.into_parts();
        if content
            .as_ref()
            .map(PreparedCheckedRichTextCheck::content_id)
            != prepared.content()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let (sealed, insertion) = self.seal_content_application(
            *prepared,
            content,
            coordinates,
            structural_edges,
            checked_callables,
            surrounding_role,
            traversal,
        )?;
        let PreparedExpressionFact::Complete(checked) = &sealed else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let CheckedExpressionResolution::ContentApplication(application) = checked.resolution()
        else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if insertion.site().raw() != reference.expression()
            || insertion.site().id() != application.id()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        self.facts
            .publish_sealed_content_application(replacement, sealed)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        traversal.leave(reference.expression());
        Ok(CheckedDialogueToken::ContentInsert(insertion))
    }

    fn seal_dialogue_effect_site(
        &self,
        site: PreparedDialogueEffectSite,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
        structural_edges: &super::super::match_edges::CheckedStructuralEdgeDraft,
        checked_callables: &crate::callable::CheckedCallableCatalog,
    ) -> Result<CheckedDialogueEffectSite, FinalSemanticAnalysisError> {
        let (id, trigger, prepared) = site.into_parts();
        let effect = self.seal_evaluated_effect(prepared)?;
        let application = self
            .facts
            .calls()
            .get(&effect.application().raw().expression())
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if application.core().application_site() != effect.application()
            || application.digest() != effect.application_digest()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let join = crate::callable::validate_selected_application(application, checked_callables)
            .map_err(|error| {
            FinalSemanticAnalysisError::CheckedCallableJoin(Box::new(error))
        })?;
        let effects = application
            .core()
            .solution()
            .instantiate_effect_row(join.schema_effects())
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let captures = self.checked_dialogue_effect_captures(
            effect.site_root(),
            coordinates,
            structural_edges,
        )?;
        Ok(CheckedDialogueEffectSite::new(
            id,
            trigger,
            effects,
            Box::new(effect),
            captures,
        ))
    }

    /// Seals the zero-argument reveal callback's free-local ABI from the one
    /// selected owning-edge authority. The compiler consumes these rows and
    /// never reopens HIR to rediscover captures.
    fn checked_dialogue_effect_captures(
        &self,
        root: ExprId,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
        structural_edges: &super::super::match_edges::CheckedStructuralEdgeDraft,
    ) -> Result<Box<[CheckedDialogueEffectCapture]>, FinalSemanticAnalysisError> {
        let root_path = coordinates
            .expression_evidence(root)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?
            .into_coordinate();
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        let mut captured = BTreeSet::<LocalId>::new();
        let mut captures = Vec::new();
        while let Some(owner) = pending.pop() {
            if !visited.insert(owner) {
                continue;
            }
            let checked = self
                .facts
                .expressions()
                .get(&owner)
                .and_then(PreparedExpressionFact::complete)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            if let Some(local) = checked.execution_local_use() {
                let ty = self
                    .facts
                    .locals()
                    .get(&local)
                    .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
                if checked.value_type() != Some(ty) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let origin = coordinates
                    .binding(local)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                if !origin.path().is_at_or_below(&root_path) && captured.insert(local) {
                    captures.push(CheckedDialogueEffectCapture::new(local, origin, ty.clone()));
                }
            }
            let children = structural_edges
                .expression_children(owner)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            pending.extend(children.iter().rev().map(|(child, _)| *child));
        }
        Ok(captures.into_boxed_slice())
    }

    /// Seals one prepared evaluated effect for either an ordinary statement
    /// or a dialogue line-plan site after final call applications exist.
    pub(crate) fn seal_evaluated_effect(
        &self,
        prepared: PreparedEvaluatedEffect,
    ) -> Result<CheckedEvaluatedEffect, FinalSemanticAnalysisError> {
        let (root, site, schema, disposition) = prepared.into_parts();
        if site != CheckedCallSite::HirCall(self.terminal_effect_call(root)?) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let application = self
            .facts
            .calls()
            .get(&site.expression())
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        self.validate_terminal_effect_application(application, site, schema, disposition)?;

        let mut operands = effect_operands(application, schema, disposition)?;
        let operation = match disposition {
            CallableEvaluatedEffect::Log(level) => CheckedEvaluatedEffectOperation::Log {
                level,
                message: operands
                    .message
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                fields: std::mem::take(&mut operands.fields).into_boxed_slice(),
            },
            CallableEvaluatedEffect::SignalWrite => CheckedEvaluatedEffectOperation::SignalWrite {
                target: operands
                    .target
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                value: operands
                    .value
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            },
            CallableEvaluatedEffect::MetricWrite => CheckedEvaluatedEffectOperation::MetricWrite {
                target: operands
                    .target
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                value: operands
                    .value
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            },
            CallableEvaluatedEffect::EmitEvent => CheckedEvaluatedEffectOperation::EmitEvent {
                event: operands
                    .event
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                fields: std::mem::take(&mut operands.fields).into_boxed_slice(),
            },
            CallableEvaluatedEffect::Panic => CheckedEvaluatedEffectOperation::Panic {
                message: operands
                    .message
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            },
            CallableEvaluatedEffect::Fail => CheckedEvaluatedEffectOperation::Fail {
                message: operands
                    .message
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            },
            CallableEvaluatedEffect::Bail => CheckedEvaluatedEffectOperation::Bail {
                message: operands
                    .message
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            },
            CallableEvaluatedEffect::Ensure => CheckedEvaluatedEffectOperation::Ensure {
                condition: operands
                    .condition
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                message: operands
                    .message
                    .take()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            },
            CallableEvaluatedEffect::Drop(operation) => {
                self.seal_drop_operation(application, operation, schema, &mut operands)?
            }
        };
        operands.finish()?;
        Ok(CheckedEvaluatedEffect::new(
            application.core().application_site().clone(),
            application.digest(),
            application
                .result()
                .value_type()
                .cloned()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            root,
            operation,
        ))
    }

    fn terminal_effect_call(&self, root: ExprId) -> Result<ExprId, FinalSemanticAnalysisError> {
        match self
            .module(root.module())?
            .resolve_expr(root)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind()
        {
            HirExprKind::Call(_) => Ok(root),
            HirExprKind::Pipe(pipe) => {
                let Some(crate::final_analysis::PreparedExpressionFact::Complete(checked)) =
                    self.facts.expressions().get(&root)
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let CheckedExpressionResolution::Pipe(checked_pipe) = checked.resolution() else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                if checked_pipe.lookup_left() != pipe.left()
                    || checked_pipe.lookup_right() != pipe.right()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                Ok(checked_pipe.lookup_right())
            }
            _ => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
        }
    }

    fn validate_terminal_effect_application(
        &self,
        application: &CheckedCallApplication,
        site: CheckedCallSite,
        schema: CallableSignatureSchemaDigest,
        disposition: CallableEvaluatedEffect,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let core = application.core();
        let selected = core.candidates().selected();
        if core.site() != site
            || selected.schema().semantic_digest() != schema
            || selected.schema().evaluated_effect() != Some(disposition)
            || selected.call_group() != core.current_group()
            || selected
                .base()
                .next_group_for(core.current_group())
                .is_some()
            || !matches!(application.result(), CheckedCallResult::Value(_))
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(())
    }

    fn seal_drop_operation(
        &self,
        application: &CheckedCallApplication,
        operation: DropCallableId,
        schema: CallableSignatureSchemaDigest,
        operands: &mut EvaluatedEffectOperands,
    ) -> Result<CheckedEvaluatedEffectOperation, FinalSemanticAnalysisError> {
        if operation == DropCallableId::OnDrop || !operands.fields.is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let target = operands
            .target
            .take()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let invocation = match operation {
            DropCallableId::Drop => {
                if !matches!(
                    application.core().candidates().selected().state(),
                    ResolvedCallableState::Base
                ) || operands.policy.is_some()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                CheckedDropInvocation::Drop
            }
            DropCallableId::DropOptional => {
                if !matches!(
                    application.core().candidates().selected().state(),
                    ResolvedCallableState::Base
                ) || operands.policy.is_some()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                CheckedDropInvocation::DropOptional
            }
            DropCallableId::DropWithPolicy => {
                let source = match application.core().candidates().selected().state() {
                    ResolvedCallableState::Base => operands
                        .policy
                        .take()
                        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                    ResolvedCallableState::Continuation(continuation) => {
                        if operands.policy.is_some() {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        }
                        let prefix = self.checked_continuation_prefix(
                            application,
                            continuation,
                            schema,
                            CallableEvaluatedEffect::Drop(operation),
                        )?;
                        let mut prefix_operands = effect_operands(
                            prefix,
                            schema,
                            CallableEvaluatedEffect::Drop(operation),
                        )?;
                        let source = prefix_operands
                            .policy
                            .take()
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        prefix_operands.finish()?;
                        source
                    }
                };
                let source = CheckedDropPolicySource::try_new(source)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let policy = self.checked_explicit_drop_policy(source.operand())?;
                CheckedDropInvocation::DropWithPolicy { source, policy }
            }
            DropCallableId::OnDrop => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        };
        Ok(CheckedEvaluatedEffectOperation::Drop { target, invocation })
    }

    fn checked_continuation_prefix<'facts>(
        &'facts self,
        terminal: &CheckedCallApplication,
        continuation: &Arc<CheckedCallContinuation>,
        schema: CallableSignatureSchemaDigest,
        disposition: CallableEvaluatedEffect,
    ) -> Result<&'facts CheckedCallApplication, FinalSemanticAnalysisError> {
        let terminal_core = terminal.core();
        let terminal_selected = terminal_core.candidates().selected();
        let prefix = self
            .facts
            .calls()
            .get(&continuation.prefix_call_site().expression())
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let prefix_core = prefix.core();
        let prefix_selected = prefix_core.candidates().selected();
        let CheckedCallResult::Continuation(prefix_continuation) = prefix.result() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };

        if terminal_core.current_group() != continuation.next_group()
            || terminal_selected.call_group() != continuation.next_group()
            || !Arc::ptr_eq(terminal_selected.base(), continuation.base())
            || prefix_core.site() != continuation.prefix_call_site()
            || prefix_core.stable_site() != continuation.prefix_application_site()
            || prefix_core.digest() != continuation.prefix_application_core()
            || !Arc::ptr_eq(prefix_selected.base(), continuation.base())
            || !Arc::ptr_eq(prefix_core.solution(), continuation.inherited_solution())
            || prefix_core.current_group() != continuation.inherited_solution().completed_group()
            || continuation
                .base()
                .next_group_for(prefix_core.current_group())
                != Some(continuation.next_group())
            || continuation.inherited_solution().base() != continuation.base().digest()
            || continuation.inherited_solution().schema() != schema
            || terminal_core.solution().base() != continuation.base().digest()
            || terminal_core.solution().schema() != schema
            || terminal_core.solution().completed_group() != continuation.next_group()
            || !Arc::ptr_eq(prefix_continuation, continuation)
            || prefix_selected.schema().semantic_digest() != schema
            || prefix_selected.schema().evaluated_effect() != Some(disposition)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(prefix)
    }

    fn checked_explicit_drop_policy(
        &self,
        source: &CheckedEvaluatedEffectOperand,
    ) -> Result<CheckedExplicitDropPolicy, FinalSemanticAnalysisError> {
        let CheckedCallArgumentSlotSource::Expression(owner) = source.source().raw() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let expression = self.facts.expressions().get(&owner);
        if expression.is_some_and(|expression| expression.value_type() != Some(source.ty())) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if let Some(CheckedExpressionResolution::Value(CheckedValueResolution::Registered(value))) =
            expression.and_then(PreparedExpressionFact::checked_resolution)
        {
            let binding = value
                .environment_binding()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            return match self
                .catalogs
                .world
                .environment()
                .typecheck_env()
                .standard_environment_value(binding)
            {
                Some(StandardEnvironmentValue::DropPolicy(StandardDropPolicyValue::Stop {
                    fade,
                })) => Ok(CheckedExplicitDropPolicy::Stop {
                    fade: CheckedDropFade::Constant(fade),
                }),
                None => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
            };
        }
        if let Some(PreparedExpressionFact::Variant(variant)) = expression {
            if &variant.owner().ty() != source.ty()
                || variant
                    .owner()
                    .case(variant.selected_ordinal())
                    .is_none_or(|case| case.payload().is_some())
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let case = self
                .catalogs
                .world
                .environment()
                .typecheck_env()
                .standard_drop_policy_case_for_type(source.ty(), variant.selected_ordinal())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            return checked_unit_drop_policy(case)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily);
        }

        let constructor = self
            .facts
            .calls()
            .get(&owner)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let core = constructor.core();
        let selected = core.candidates().selected();
        let CheckedCallResult::Value(result) = constructor.result() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let ResolvedCallableStableIdentity::Language(
            CheckedLanguageCallableIdentity::EnumConstructor {
                owner: constructor_owner,
                case,
            },
        ) = selected.base().authority().stable()
        else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if core.site() != CheckedCallSite::HirCall(owner)
            || core.stable_site() != source.source().coordinate()
            || result != source.ty()
            || *constructor_owner != source.ty().semantic_identity_digest()?
            || !matches!(selected.state(), ResolvedCallableState::Base)
            || selected.call_group() != core.current_group()
            || selected
                .base()
                .next_group_for(core.current_group())
                .is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let case = self
            .catalogs
            .world
            .environment()
            .typecheck_env()
            .standard_drop_policy_case_for_type(source.ty(), *case)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let runtime_operands = core.runtime_operands();
        match (case, runtime_operands.is_empty()) {
            (case, true) => {
                checked_unit_drop_policy(case).ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
            (StandardDropPolicyCase::Stop, false) => {
                let [operand] = runtime_operands.as_ref() else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let CheckedCallRuntimeOperand::Argument { slot, .. } = *operand else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                if !matches!(
                    slot.destination(),
                    CheckedCallOperandDestination::Parameter(_)
                ) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let fade = CheckedDropFadeOperand::try_new(checked_operand(
                    slot.source(),
                    slot.inferred(),
                ))
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                Ok(CheckedExplicitDropPolicy::Stop {
                    fade: CheckedDropFade::Operand(fade),
                })
            }
            (_, false) => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
        }
    }
}

fn checked_unit_drop_policy(case: StandardDropPolicyCase) -> Option<CheckedExplicitDropPolicy> {
    match case {
        StandardDropPolicyCase::Cancel => Some(CheckedExplicitDropPolicy::Cancel),
        StandardDropPolicyCase::Finish => Some(CheckedExplicitDropPolicy::Finish),
        StandardDropPolicyCase::Release => Some(CheckedExplicitDropPolicy::Release),
        StandardDropPolicyCase::Detach => Some(CheckedExplicitDropPolicy::Detach),
        StandardDropPolicyCase::Stop => None,
    }
}

fn effect_operands(
    application: &CheckedCallApplication,
    schema: CallableSignatureSchemaDigest,
    disposition: CallableEvaluatedEffect,
) -> Result<EvaluatedEffectOperands, FinalSemanticAnalysisError> {
    let selected = application.core().candidates().selected();
    if selected.schema().semantic_digest() != schema
        || selected.schema().evaluated_effect() != Some(disposition)
    {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    let mut operands = EvaluatedEffectOperands::default();
    let mut open_arguments = BTreeSet::new();
    for runtime_operand in application.core().runtime_operands() {
        match runtime_operand {
            CheckedCallRuntimeOperand::Receiver { source, ty, .. } => {
                let receiver = selected
                    .schema()
                    .extension_receiver()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let coordinate =
                    CallableParameterCoordinate::new(receiver.group(), receiver.parameter());
                let role = disposition
                    .operand_role(coordinate)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                operands.insert(role, checked_operand(source, ty))?;
            }
            CheckedCallRuntimeOperand::Argument { slot, .. } => match slot.destination() {
                CheckedCallOperandDestination::Parameter(coordinate) => {
                    let role = disposition
                        .operand_role(*coordinate)
                        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                    operands.insert(role, checked_operand(slot.source(), slot.inferred()))?;
                }
                CheckedCallOperandDestination::Open(open) => {
                    if !disposition.accepts_open_fields()
                        || open.schema() != schema
                        || !open_arguments.insert(open.clone())
                    {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    operands.fields.push(CheckedEffectField::new(
                        open.clone(),
                        checked_operand(slot.source(), slot.inferred()),
                    ));
                }
            },
            CheckedCallRuntimeOperand::AttachedContent { .. } => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
    }
    Ok(operands)
}

fn checked_operand(
    source: &crate::callable::CheckedCallExecutionSource,
    ty: &crate::types::TypeKind,
) -> CheckedEvaluatedEffectOperand {
    CheckedEvaluatedEffectOperand::new(source.clone(), ty.clone())
}
