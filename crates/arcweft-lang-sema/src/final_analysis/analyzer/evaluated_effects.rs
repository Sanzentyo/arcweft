//! Post-call sealing for runtime-observable expression-statement effects.
//!
//! Preparation retains only callable-owned identity. This pass runs after C1
//! and projects operands exclusively from the final checked applications.

use std::{collections::BTreeSet, sync::Arc};

use crate::{
    callable::{
        CallableEvaluatedEffect, CallableEvaluatedEffectOperandRole, CallableParameterCoordinate,
        CallableSignatureSchemaDigest, CheckedCallApplication, CheckedCallArgumentSlotSource,
        CheckedCallContinuation, CheckedCallOperandDestination, CheckedCallResult,
        CheckedCallRuntimeOperand, CheckedCallRuntimeOperandOrder, CheckedCallSite,
        CheckedLanguageCallableIdentity, DropCallableId, ResolvedCallableStableIdentity,
        ResolvedCallableState,
    },
    env::{StandardDropPolicyCase, StandardDropPolicyValue, StandardEnvironmentValue},
    final_analysis::{
        CheckedDialogueEffectSite, CheckedDialogueLinePlan, CheckedDropFade,
        CheckedDropFadeOperand, CheckedDropInvocation, CheckedDropPolicySource, CheckedEffectField,
        CheckedEvaluatedEffect, CheckedEvaluatedEffectOperand, CheckedEvaluatedEffectOperation,
        CheckedExplicitDropPolicy, CheckedExpression, CheckedExpressionResolution,
        CheckedValueResolution, PreparedDialogueApplication, PreparedDialogueEffectSite,
        PreparedEvaluatedEffect, PreparedExpressionFact, PreparedStatementPayload,
    },
    semantic_coordinate::SemanticCoordinateIndex,
};

use crate::checked_rich_text::{
    CheckedDialogueContent, CheckedDialogueMark, CheckedDialogueToken, CheckedRichTextAction,
    CheckedRichTextReport, CheckedRichTextTag, PreparedCheckedDialogueMarkCatalog,
    PreparedCheckedDialogueToken, PreparedCheckedRichTextAction, PreparedCheckedRichTextReport,
};

use arcweft_lang_hir::{expr::HirExprKind, identity::ExprId, module::HirModule};

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
            HirExprKind::Pipe(authored_pipe) => {
                let Some(CheckedExpressionResolution::Pipe(checked_pipe)) = self
                    .facts
                    .expressions()
                    .get(&expression)
                    .and_then(PreparedExpressionFact::checked_resolution)
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                if checked_pipe.left() != authored_pipe.left()
                    || checked_pipe.right() != authored_pipe.right()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                checked_pipe.right()
            }
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
            CheckedCallSite::HirCall(call_owner),
            application.selected().schema().semantic_digest(),
            disposition,
        )))
    }

    pub(super) fn finalize_evaluated_effects(
        &mut self,
        input: &mut FinalSemanticAnalysisInput,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
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
            let taken = self
                .facts
                .take_dialogue_application(owner)
                .map_err(FinalSemanticAnalysisError::from)?;
            let (prepared, markers, replacement) = taken.into_parts();
            let sealed = self.seal_dialogue_application(prepared, markers, coordinates)?;
            self.facts
                .publish_sealed_dialogue_application(replacement, sealed)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        }
        if !self.facts.dialogue_mark_catalogs_are_empty() {
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
        &self,
        prepared: PreparedDialogueApplication,
        markers: PreparedCheckedDialogueMarkCatalog,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<PreparedExpressionFact, FinalSemanticAnalysisError> {
        let (shell, target, application_patch, rich_text, line_plan, line_result, nested_path) =
            prepared.into_parts();
        let rich_text = self.seal_checked_rich_text(*rich_text, markers, coordinates)?;
        let effect_sites = line_plan.into_parts();
        let effect_sites = effect_sites
            .into_vec()
            .into_iter()
            .map(|site| self.seal_dialogue_effect_site(site))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let line_plan = CheckedDialogueLinePlan::new(effect_sites);
        let (ty, type_selection, effects) = shell.into_parts();
        if ty != crate::types::TypeKind::DialogueLine(Box::new(line_result.clone())) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let resolution = CheckedExpressionResolution::DialogueApplication {
            target,
            application_patch,
            rich_text: Box::new(rich_text),
            line_plan,
            line_result,
        };
        let expression = CheckedExpression::new(ty, type_selection, effects, resolution);
        Ok(match nested_path {
            Some(evidence) => {
                PreparedExpressionFact::Complete(expression.with_nested_path_evidence(evidence))
            }
            None => PreparedExpressionFact::Complete(expression),
        })
    }

    fn seal_checked_rich_text(
        &self,
        prepared: PreparedCheckedRichTextReport,
        mut markers: PreparedCheckedDialogueMarkCatalog,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<CheckedRichTextReport, FinalSemanticAnalysisError> {
        let (content, diagnostics) = prepared.into_parts();
        let (content_id, tokens, diagnostics_complete) = content.into_parts();
        if !diagnostics_complete || !diagnostics.is_empty() || markers.content() != content_id {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let mut marker_count = 0_usize;
        let tokens = tokens
            .into_vec()
            .into_iter()
            .map(|token| {
                Ok(match token {
                    PreparedCheckedDialogueToken::Text(text) => CheckedDialogueToken::Text(text),
                    PreparedCheckedDialogueToken::RawText(text) => {
                        CheckedDialogueToken::RawText(text)
                    }
                    PreparedCheckedDialogueToken::Escape(value) => {
                        CheckedDialogueToken::Escape(value)
                    }
                    PreparedCheckedDialogueToken::Ruby { base, ruby } => {
                        CheckedDialogueToken::Ruby { base, ruby }
                    }
                    PreparedCheckedDialogueToken::Open(tag) => {
                        let (id, owner, action, source) = tag.into_parts();
                        let action = match action {
                            PreparedCheckedRichTextAction::Control { action, fields } => {
                                CheckedRichTextAction::Control { action, fields }
                            }
                            PreparedCheckedRichTextAction::DirectStyle {
                                owner,
                                action,
                                fields,
                            } => CheckedRichTextAction::DirectStyle {
                                owner,
                                action,
                                fields,
                            },
                            PreparedCheckedRichTextAction::Style {
                                owner,
                                action,
                                fields,
                            } => CheckedRichTextAction::Style {
                                owner,
                                action,
                                fields,
                            },
                            PreparedCheckedRichTextAction::Layout {
                                owner,
                                action,
                                fields,
                            } => CheckedRichTextAction::Layout {
                                owner,
                                action,
                                fields,
                            },
                            PreparedCheckedRichTextAction::Transform {
                                owner,
                                action,
                                fields,
                            } => CheckedRichTextAction::Transform {
                                owner,
                                action,
                                fields,
                            },
                            PreparedCheckedRichTextAction::Object { action, fields } => {
                                CheckedRichTextAction::Object { action, fields }
                            }
                            PreparedCheckedRichTextAction::BuiltinFx {
                                effect,
                                phase,
                                fields,
                            } => CheckedRichTextAction::BuiltinFx {
                                effect,
                                phase,
                                fields,
                            },
                            PreparedCheckedRichTextAction::Host {
                                owner,
                                action,
                                fields,
                            } => CheckedRichTextAction::Host {
                                owner,
                                action,
                                fields,
                            },
                            PreparedCheckedRichTextAction::Marker => {
                                let mark = markers
                                    .take(id)
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
                        CheckedDialogueToken::Open(CheckedRichTextTag::new(
                            id, owner, action, source,
                        ))
                    }
                    PreparedCheckedDialogueToken::Close(close) => {
                        CheckedDialogueToken::Close(close)
                    }
                    PreparedCheckedDialogueToken::InvalidTag { .. } => {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    PreparedCheckedDialogueToken::Interpolation(expression) => {
                        CheckedDialogueToken::Interpolation(expression)
                    }
                    PreparedCheckedDialogueToken::LineBreak(kind) => {
                        CheckedDialogueToken::LineBreak(kind)
                    }
                })
            })
            .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?;
        let module = self.module(content_id.owner().module())?;
        let expression = module
            .resolve_expr(content_id.owner())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::DialogueContentApplication(application) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if application.content().id() != content_id
            || application.content().marks().len() != marker_count
            || !markers.is_empty()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(CheckedRichTextReport::new(
            CheckedDialogueContent::new(content_id, tokens, true),
            diagnostics.into_vec(),
        ))
    }

    fn seal_dialogue_effect_site(
        &self,
        site: PreparedDialogueEffectSite,
    ) -> Result<CheckedDialogueEffectSite, FinalSemanticAnalysisError> {
        let (id, trigger, expression, prepared) = site.into_parts();
        let expected_call = match self
            .module(expression.module())?
            .resolve_expr(expression)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind()
        {
            HirExprKind::Call(_) => expression,
            HirExprKind::Pipe(pipe) => {
                let Some(crate::final_analysis::PreparedExpressionFact::Complete(checked)) =
                    self.facts.expressions().get(&expression)
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let CheckedExpressionResolution::Pipe(checked_pipe) = checked.resolution() else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                if checked_pipe.left() != pipe.left() || checked_pipe.right() != pipe.right() {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                checked_pipe.right()
            }
            _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
        };
        if prepared.site() != CheckedCallSite::HirCall(expected_call) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let effect = self.seal_evaluated_effect(prepared)?;
        Ok(CheckedDialogueEffectSite::new(
            id,
            trigger,
            Box::new(effect),
        ))
    }

    /// Seals one prepared evaluated effect for either an ordinary statement
    /// or a dialogue line-plan site after final call applications exist.
    pub(crate) fn seal_evaluated_effect(
        &self,
        prepared: PreparedEvaluatedEffect,
    ) -> Result<CheckedEvaluatedEffect, FinalSemanticAnalysisError> {
        let (site, schema, disposition) = prepared.into_parts();
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
            operation,
        ))
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
        if expression.is_some_and(|expression| expression.ty() != source.ty()) {
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
        if let Some(CheckedExpressionResolution::Variant(variant)) =
            expression.and_then(PreparedExpressionFact::checked_resolution)
        {
            if variant.owner().semantic_type() != source.ty().semantic_identity_digest()
                || !variant.selected().payload().is_unit()
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let case = self
                .catalogs
                .world
                .environment()
                .typecheck_env()
                .standard_drop_policy_case_for_type(source.ty(), variant.ordinal())
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
            || *constructor_owner != source.ty().semantic_identity_digest()
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
        let runtime_operands = core
            .execution()
            .ordered_runtime_operands(CheckedCallRuntimeOperandOrder::Source);
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
    for runtime_operand in application
        .core()
        .execution()
        .ordered_runtime_operands(CheckedCallRuntimeOperandOrder::Source)
    {
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
