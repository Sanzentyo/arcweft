use super::{
    Engine, FlowFiberStatus, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr, RuntimeExprMatchArm,
    RuntimeMatchArm, RuntimeMatchSelection, RuntimePattern, RuntimeSeq, RuntimeStepOutput,
    RuntimeValue, evaluate_binary, evaluate_unary, runtime_sequence_dense_i64,
    runtime_sequence_from_literal_values, runtime_sequence_repeat_value, runtime_sequence_values,
    runtime_value_into_sequence_values, runtime_value_label, sum_i64_sequence_ref,
};
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner};
use crate::plan::{
    FlowRuntimeId, RuntimePlanTypeDeclaration, RuntimePlanTypeProjection, RuntimePureInputType,
    RuntimePureOutputType,
};
use crate::pure::{
    RuntimeCallBackend, RuntimeExternalCallContext, RuntimeI64Args, VmRuntimePureCallBackend,
};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::RuntimeBinaryOp;
use crate::value::{
    RuntimeAgentExpr, RuntimeAgentValue, RuntimeCallArgumentMode, RuntimeCallTarget,
    RuntimeExprKind, RuntimeFieldProjection, RuntimeIntrinsic, evaluate_core_iterator_intrinsic,
};
use crate::value::{RuntimeLocalBinding, RuntimeNominalRecordExpr};
use crate::value::{
    RuntimeReductionValue, evaluate_capacity_intrinsic, evaluate_core_range_intrinsic,
    evaluate_index_intrinsic, evaluate_std_float_intrinsic, evaluate_string_intrinsic,
};
use std::sync::Arc;

mod callable;
mod calls;
mod character_dialogue;

impl Engine {
    pub(super) fn evaluate_let_with_backend(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        match self
            .evaluate_expr_with_backend(expr, pure_backend)
            .and_then(|value| self.try_bind_pattern_owned(pattern, value))
        {
            Ok(None) => {}
            Ok(Some(value)) => {
                self.fail_eval(
                    RuntimeEvalError::PatternMismatch(runtime_value_label(&value)),
                    output,
                );
            }
            Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
        }
    }

    pub(super) fn evaluate_if_let_with_backend(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<Vec<RuntimeLocalBinding>>, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        if !crate::pattern::inspect_runtime_pattern_owned(&self.plan, pattern, &value)? {
            return Ok(None);
        }
        if let Some(guard) = guard {
            let projected = crate::pattern::prepare_runtime_pattern_guard_bindings(
                &self.plan, pattern, &value, guard,
            )?;
            let matched = self.with_temp_bindings(projected, |this| {
                this.evaluate_bool_with_backend(guard, pure_backend)
            })?;
            if !matched {
                return Ok(None);
            }
        }
        let bindings = crate::pattern::match_runtime_pattern_owned(&self.plan, pattern, value)?
            .expect("checked owned pattern remains matched");
        Ok(Some(bindings))
    }

    pub(super) fn evaluate_match_with_backend(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: Vec<RuntimeMatchArm>,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeMatchSelection, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(scrutinee, pure_backend)?;
        for arm in arms {
            if !crate::pattern::inspect_runtime_pattern_owned(&self.plan, &arm.pattern, &value)? {
                continue;
            }
            if let Some(guard) = arm.guard.as_ref() {
                let projected = crate::pattern::prepare_runtime_pattern_guard_bindings(
                    &self.plan,
                    &arm.pattern,
                    &value,
                    guard,
                )?;
                if !self.with_temp_bindings(projected, |this| {
                    this.evaluate_bool_with_backend(guard, pure_backend)
                })? {
                    continue;
                }
            }
            let bindings =
                crate::pattern::match_runtime_pattern_owned(&self.plan, &arm.pattern, value)?
                    .expect("checked owned pattern remains matched");
            return Ok(Some((bindings, arm.ops)));
        }
        Ok(None)
    }

    pub(super) fn evaluate_expr(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut pure_backend = VmRuntimePureCallBackend::default();
        self.evaluate_expr_with_backend(expr, &mut pure_backend)
    }

    pub(super) fn evaluate_expr_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        match expr.kind() {
            RuntimeExprKind::Value(value) => value
                .ownership()
                .permits_copy()
                .then(|| value.clone())
                .ok_or(RuntimeEvalError::AffineLiteralCopy),
            RuntimeExprKind::Agent(agent) => self.evaluate_agent_expr(agent, pure_backend),
            RuntimeExprKind::Local(read) => self.fiber.env.read(read),
            RuntimeExprKind::SequencePopFront { place } => {
                self.fiber.env.pop_sequence_front(*place).map(|value| {
                    value.map_or_else(RuntimeValue::option_none, RuntimeValue::option_some)
                })
            }
            RuntimeExprKind::SequencePush { place, value } => {
                let value = self.evaluate_expr_with_backend(value, pure_backend)?;
                self.fiber.env.push_vector_item(*place, value)?;
                Ok(RuntimeValue::Unit)
            }
            RuntimeExprKind::SequencePopBack { place } => {
                self.fiber.env.pop_vector_item(*place).map(|value| {
                    value.map_or_else(RuntimeValue::option_none, RuntimeValue::option_some)
                })
            }
            RuntimeExprKind::EntityRef(target) => Ok(RuntimeValue::EntityRef(target.clone())),
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => self.evaluate_let_expr(*binding, expr, body, pure_backend),
            RuntimeExprKind::Scope { identity, body } => {
                self.fiber.env.push_scope_with_identity(identity.clone());
                let result = self.evaluate_expr_with_backend(body, pure_backend);
                self.fiber.env.pop_scope();
                result
            }
            RuntimeExprKind::Tuple(_)
            | RuntimeExprKind::BracketSeq(_)
            | RuntimeExprKind::RepeatSeq { .. }
            | RuntimeExprKind::Range { .. }
            | RuntimeExprKind::NominalRecord(_)
            | RuntimeExprKind::Variant { .. }
            | RuntimeExprKind::Field { .. }
            | RuntimeExprKind::ProjectTuple { .. }
            | RuntimeExprKind::ProjectRecord { .. }
            | RuntimeExprKind::Assign { .. } => self.evaluate_data_expr(expr, pure_backend),
            RuntimeExprKind::DialogueContent {
                template,
                values,
                effects,
            } => self.evaluate_dialogue_content_expr(*template, values, effects, pure_backend),
            RuntimeExprKind::FormatContent {
                template,
                attempt,
                operands,
                project_method,
                project_option,
            } => self.evaluate_format_content_expr(
                *template,
                *attempt,
                operands,
                *project_method,
                *project_option,
                pure_backend,
            ),
            RuntimeExprKind::CharacterDialogue {
                operation,
                target,
                fields,
            } => self.evaluate_character_dialogue_expr(
                expr.ty(),
                *operation,
                target,
                fields,
                pure_backend,
            ),
            RuntimeExprKind::Call { callee, args } => {
                self.evaluate_call_expr(callee, args, expr.ty(), pure_backend)
            }
            RuntimeExprKind::MakeCallable { state, captures } => {
                self.evaluate_callable_expr(*state, captures, pure_backend)
            }
            RuntimeExprKind::SpecializeCallable {
                value,
                specialization,
            } => self.evaluate_specialize_callable_expr(value, *specialization, pure_backend),
            RuntimeExprKind::ApplyGroup { callee, args } => {
                self.evaluate_apply_expr(callee, args, pure_backend)
            }
            RuntimeExprKind::TraitCall {
                callable,
                receiver,
                receiver_mode,
                args,
            } => self
                .evaluate_trait_method_call(*callable, *receiver_mode, receiver, args, pure_backend)
                .map(|outcome| outcome.value),
            RuntimeExprKind::PureCall { helper, args } => {
                self.evaluate_pure_call_expr(*helper, args, pure_backend)
            }
            RuntimeExprKind::StandardMap {
                family,
                order,
                mapping,
                source,
            } => self.evaluate_standard_map_expr(*family, *order, mapping, source, pure_backend),
            RuntimeExprKind::Sum { source } => self.evaluate_sum_expr(source, pure_backend),
            RuntimeExprKind::Unary { op, expr } => {
                self.evaluate_unary_expr(*op, expr, pure_backend)
            }
            RuntimeExprKind::Binary { lhs, op, rhs } => {
                self.evaluate_binary_expr(lhs, *op, rhs, pure_backend)
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => self.evaluate_if_expr(condition, then_expr, else_expr, pure_backend),
            RuntimeExprKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => self.evaluate_if_let_expr(
                pattern,
                expr,
                guard.as_deref(),
                then_expr,
                else_expr,
                pure_backend,
            ),
            RuntimeExprKind::Match { scrutinee, arms } => {
                self.evaluate_match_expr(scrutinee, arms, pure_backend)
            }
            RuntimeExprKind::ReductionUnchanged { state } => {
                self.evaluate_reduction_unchanged(expr.ty(), state, pure_backend)
            }
        }
    }

    fn evaluate_unary_expr(
        &mut self,
        op: crate::value::RuntimeUnaryOp,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        evaluate_unary(op, value)
    }

    fn evaluate_binary_expr(
        &mut self,
        lhs: &RuntimeExpr,
        op: RuntimeBinaryOp,
        rhs: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let lhs = self.evaluate_expr_with_backend(lhs, pure_backend)?;
        let rhs = self.evaluate_expr_with_backend(rhs, pure_backend)?;
        evaluate_binary(lhs, op, rhs)
    }

    fn evaluate_agent_expr(
        &mut self,
        agent: &RuntimeAgentExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut operands = Vec::new();
        if let Some(choice) = agent.choice() {
            operands.push(RuntimeValue::String(choice.as_str().to_owned()));
        }
        for operand in agent.operands() {
            operands.push(self.evaluate_expr_with_backend(operand, pure_backend)?);
        }
        RuntimeAgentValue::try_construct(agent.constructor(), operands)
            .map(RuntimeValue::Agent)
            .map_err(|error| RuntimeEvalError::AgentConstruction(error.to_string()))
    }

    fn evaluate_data_expr(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        match expr.kind() {
            RuntimeExprKind::Tuple(items) => items
                .iter()
                .map(|item| self.evaluate_expr_with_backend(item, pure_backend))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Tuple),
            RuntimeExprKind::BracketSeq(items) => {
                self.evaluate_bracket_seq_expr(items, pure_backend)
            }
            RuntimeExprKind::RepeatSeq { value, len } => {
                self.evaluate_repeat_seq_expr(value, *len, pure_backend)
            }
            RuntimeExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                self.evaluate_range_expr(start.as_deref(), end.as_deref(), *inclusive, pure_backend)
            }
            RuntimeExprKind::NominalRecord(record) => {
                self.evaluate_nominal_record_expr(expr.ty(), record, pure_backend)
            }
            RuntimeExprKind::Variant { ordinal, payload } => {
                self.evaluate_variant_expr(expr.ty(), *ordinal, payload.as_deref(), pure_backend)
            }
            RuntimeExprKind::Field { target, field } => {
                self.evaluate_field_expr(target, field, pure_backend)
            }
            RuntimeExprKind::ProjectTuple { target, ordinal } => {
                self.evaluate_project_tuple_expr(target, *ordinal, pure_backend)
            }
            RuntimeExprKind::ProjectRecord { target, ordinal } => {
                self.evaluate_project_record_expr(target, *ordinal, pure_backend)
            }
            RuntimeExprKind::Assign { place, expr, body } => {
                self.evaluate_assign_expr(place, expr, body, pure_backend)
            }
            _ => unreachable!("data expression helper received non-data expression"),
        }
    }

    fn evaluate_dialogue_content_expr(
        &mut self,
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
        values: &[RuntimeExpr],
        effects: &[crate::value::RuntimeDialogueContentEffectBindingExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let manifest = self
            .plan
            .dialogue_content_templates()
            .get(template)
            .cloned()
            .ok_or(RuntimeEvalError::MissingDialogueTemplateManifest { template })?;
        let evaluated = values
            .iter()
            .map(|expression| self.evaluate_expr_with_backend(expression, pure_backend))
            .collect::<Result<Vec<_>, RuntimeEvalError>>()?;
        if evaluated.len() != manifest.slots().len() {
            return Err(RuntimeEvalError::DialogueContentBindingCount {
                expected: manifest.slots().len(),
                actual: evaluated.len(),
            });
        }
        let evaluated = manifest
            .slots()
            .iter()
            .zip(evaluated)
            .map(|(slot, value)| crate::plan::RuntimeDialogueValueBinding {
                slot: slot.slot(),
                role: slot.role(),
                value,
            })
            .collect::<Vec<_>>();
        if effects.len() != manifest.effects().len() {
            return Err(RuntimeEvalError::DialogueContentConstruction(format!(
                "evaluated effect count {} does not match template count {}",
                effects.len(),
                manifest.effects().len()
            )));
        }
        let mut effect_bindings = Vec::with_capacity(effects.len());
        for (index, effect) in effects.iter().enumerate() {
            let expected = manifest.effects().get(index).ok_or_else(|| {
                RuntimeEvalError::DialogueContentConstruction(
                    "dialogue content effect site is absent".to_owned(),
                )
            })?;
            if effect.site != expected.site() {
                return Err(RuntimeEvalError::DialogueContentConstruction(
                    "dialogue content effect sites are not canonical".to_owned(),
                ));
            }
            let captures = effect
                .captures
                .iter()
                .map(|capture| self.evaluate_expr_with_backend(capture, pure_backend))
                .collect::<Result<Vec<_>, RuntimeEvalError>>()?;
            let callback = crate::value::RuntimeCallableValue::try_new(
                crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
                effect.state,
                captures,
            )?;
            let remaining = callback.remaining_arity()?;
            if remaining != 0 {
                return Err(RuntimeEvalError::FunctionArgumentCount {
                    expected: 0,
                    found: remaining,
                });
            }
            effect_bindings.push(crate::value::RuntimeDialogueContentEffectBinding::new(
                effect.site,
                callback,
            ));
        }
        let artifact = self
            .plan
            .artifact()
            .ok_or(RuntimeEvalError::DialogueContentUnboundArtifact)?;
        crate::value::RuntimeDialogueContentValue::try_from_evaluated_bindings_with_effects(
            artifact,
            &manifest,
            &evaluated,
            &effect_bindings,
        )
        .map(crate::value::RuntimeDialogueContentValue::into_runtime_value)
        .map_err(|error| RuntimeEvalError::DialogueContentConstruction(error.to_string()))
    }

    fn evaluate_format_content_expr(
        &mut self,
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
        attempt: Option<crate::runtime_id::RuntimeFormatAttemptId>,
        operands: &[crate::value::RuntimeFormatContentOperand],
        project_method: Option<crate::plan::RuntimeTraitMethodId>,
        project_option: bool,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let manifest = self
            .plan
            .dialogue_content_templates()
            .get(template)
            .cloned()
            .ok_or(RuntimeEvalError::MissingDialogueTemplateManifest { template })?;
        let [slot] = manifest.slots() else {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "fmt Content template must have one Formatted slot".to_owned(),
            ));
        };
        if slot.role() != crate::plan::RuntimeDialogueValueRole::Formatted
            || slot.semantic_type()
                != crate::value::RuntimeDialogueOpaqueRole::Content.semantic_identity()
            || !manifest.effects().is_empty()
        {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "fmt Content template must have one exact Formatted/Content slot and no effects"
                    .to_owned(),
            ));
        }

        let (format_context, mut evaluated, mut first_recoverable, primary_type_id) =
            if let Some(attempt) = attempt {
                if !operands.is_empty()
                    || self
                        .plan
                        .format_attempt(attempt)
                        .is_none_or(|row| row.template() != template)
                {
                    return Err(RuntimeEvalError::DialogueContentConstruction(
                        "FormatContent attempt does not match its template".to_owned(),
                    ));
                }
                let values = self.take_format_attempt_values(attempt)?;
                (
                    values.context,
                    values.values,
                    values.first_recoverable,
                    values.primary_type,
                )
            } else {
                let mut evaluated = Vec::with_capacity(operands.len());
                let mut first_recoverable = None;
                for operand in operands {
                    let value =
                        match self.evaluate_expr_with_backend(operand.expression(), pure_backend) {
                            Ok(value) => Some(value),
                            Err(RuntimeEvalError::RecoverableExpression(failure)) => {
                                if first_recoverable.is_none() {
                                    first_recoverable = Some(failure.to_string());
                                }
                                None
                            }
                            Err(error) => return Err(error),
                        };
                    evaluated.push((operand.parameter(), value));
                }
                let primary = operands
                    .iter()
                    .find(|operand| {
                        operand.parameter() == crate::value::RuntimeFmtParameterId::Value
                    })
                    .ok_or_else(|| {
                        RuntimeEvalError::DialogueContentConstruction(
                            "fmt Content has no primary value expression".to_owned(),
                        )
                    })?;
                (
                    self.format_context.clone(),
                    evaluated,
                    first_recoverable,
                    primary.expression().ty(),
                )
            };
        let primary_type = self
            .plan
            .type_table()
            .get(primary_type_id)
            .ok_or(RuntimeEvalError::InvalidExpressionType(primary_type_id))?;
        let mut primary_kind = match primary_type.projection() {
            RuntimePlanTypeProjection::Option { item, .. } => {
                let item = self
                    .plan
                    .type_table()
                    .get(*item)
                    .ok_or(RuntimeEvalError::InvalidExpressionType(*item))?;
                crate::value::RuntimeFormatPrimaryKind::OptionScalar(item.semantic_identity())
            }
            _ if primary_type.semantic_identity()
                == crate::value::RuntimeDialogueOpaqueRole::Content.semantic_identity() =>
            {
                crate::value::RuntimeFormatPrimaryKind::Content
            }
            _ => crate::value::RuntimeFormatPrimaryKind::Scalar(primary_type.semantic_identity()),
        };
        if let Some(method_id) = project_method {
            primary_kind = if project_option {
                crate::value::RuntimeFormatPrimaryKind::OptionProjectContent
            } else {
                crate::value::RuntimeFormatPrimaryKind::ProjectContent
            };
            if first_recoverable.is_none() {
                let method = self
                    .plan
                    .trait_methods()
                    .get(method_id.0)
                    .filter(|method| method.id == method_id)
                    .cloned()
                    .ok_or(RuntimeEvalError::UnknownTraitMethod(method_id.0))?;
                let [receiver_local, context_local] = method.input_locals.as_ref() else {
                    return Err(RuntimeEvalError::DialogueContentConstruction(
                        "project DisplayText method must have receiver and context".to_owned(),
                    ));
                };
                let context_ty = self
                    .plan
                    .local_declarations()
                    .get(*context_local)
                    .ok_or(RuntimeEvalError::UnknownLocal(*context_local))?
                    .ty();
                let context_layout = crate::value::project_display_layout(&self.plan, context_ty)
                    .map_err(|error| {
                    RuntimeEvalError::DialogueContentConstruction(error.to_string())
                })?;
                let result_ty = self
                    .plan
                    .type_table()
                    .get(method.body.ty())
                    .ok_or(RuntimeEvalError::InvalidExpressionType(method.body.ty()))?;
                let RuntimePlanTypeProjection::Result { error, .. } = result_ty.projection() else {
                    return Err(RuntimeEvalError::InvalidExpressionType(method.body.ty()));
                };
                let error_layout = crate::value::project_display_layout(&self.plan, *error)
                    .map_err(|error| {
                        RuntimeEvalError::DialogueContentConstruction(error.to_string())
                    })?;
                let value_position = evaluated
                    .iter()
                    .position(|(parameter, _)| {
                        *parameter == crate::value::RuntimeFmtParameterId::Value
                    })
                    .ok_or_else(|| {
                        RuntimeEvalError::DialogueContentConstruction(
                            "project DisplayText has no primary value".to_owned(),
                        )
                    })?;
                let receiver = evaluated[value_position].1.clone().ok_or_else(|| {
                    RuntimeEvalError::DialogueContentConstruction(
                        "project DisplayText primary was not evaluated".to_owned(),
                    )
                })?;
                let receiver = if project_option {
                    match receiver.try_into_builtin_variant_case() {
                        Ok((
                            crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome,
                            Some(value),
                        )) => Some(value),
                        Ok((
                            crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionNone,
                            None,
                        )) => None,
                        _ => {
                            return Err(RuntimeEvalError::InvalidExpressionType(primary_type_id));
                        }
                    }
                } else {
                    Some(receiver)
                };
                if let Some(receiver) = receiver {
                    let receiver_ty = self
                        .plan
                        .local_declarations()
                        .get(*receiver_local)
                        .ok_or(RuntimeEvalError::UnknownLocal(*receiver_local))?
                        .ty();
                    if !self.plan.value_matches_type(receiver_ty, &receiver)? {
                        return Err(RuntimeEvalError::InvalidExpressionType(receiver_ty));
                    }
                    match crate::value::project_display_context(
                        &context_layout,
                        &format_context,
                        &evaluated,
                    )
                    .map_err(|error| {
                        RuntimeEvalError::DialogueContentConstruction(error.to_string())
                    })? {
                        Ok(context) => {
                            let outcome = self.evaluate_trait_method_values(
                                method_id,
                                crate::plan::RuntimeReceiverMode::Owned,
                                receiver,
                                vec![context],
                                pure_backend,
                            );
                            match outcome {
                                Ok(outcome) => {
                                    match crate::value::project_display_result(
                                        outcome.value,
                                        &error_layout,
                                    )
                                    .map_err(|error| {
                                        RuntimeEvalError::DialogueContentConstruction(
                                            error.to_string(),
                                        )
                                    })? {
                                        Ok(content) => {
                                            evaluated[value_position].1 = Some(if project_option {
                                                RuntimeValue::option_some(content)
                                            } else {
                                                content
                                            });
                                        }
                                        Err(reason) => first_recoverable = Some(reason),
                                    }
                                }
                                Err(RuntimeEvalError::RecoverableExpression(failure)) => {
                                    first_recoverable = Some(failure.to_string());
                                }
                                Err(error) => return Err(error),
                            }
                        }
                        Err(reason) => first_recoverable = Some(reason),
                    }
                }
            }
        } else if project_option {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "project fmt option has no selected DisplayText method".to_owned(),
            ));
        }
        let formatted = crate::value::finish_format_content_attempt(
            &format_context,
            primary_kind,
            &evaluated,
            first_recoverable.as_deref(),
        )
        .map_err(|error| RuntimeEvalError::DialogueContentConstruction(error.to_string()))?;
        let artifact = self
            .plan
            .artifact()
            .ok_or(RuntimeEvalError::DialogueContentUnboundArtifact)?;
        let binding = crate::plan::RuntimeDialogueValueBinding {
            slot: slot.slot(),
            role: slot.role(),
            value: formatted.into_runtime_value(),
        };
        crate::value::RuntimeDialogueContentValue::try_from_evaluated_bindings(
            artifact,
            &manifest,
            &[binding],
        )
        .map(crate::value::RuntimeDialogueContentValue::into_runtime_value)
        .map_err(|error| RuntimeEvalError::DialogueContentConstruction(error.to_string()))
    }

    fn evaluate_bracket_seq_expr(
        &mut self,
        items: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Some((helper_id, arity)) = self.bracket_seq_i64_batch_shape(items) {
            let mut flat_inputs = std::mem::take(&mut self.pure_i64_batch_inputs);
            let collect_result =
                self.collect_i64_pure_batch_inputs(items, arity, pure_backend, &mut flat_inputs);
            if let Err(error) = collect_result {
                self.pure_i64_batch_inputs = flat_inputs;
                return Err(error);
            }
            let batch_result = self.call_i64_flat_batch_with_outputs(
                helper_id,
                &flat_inputs,
                arity,
                items.len(),
                pure_backend,
                <[i64]>::to_vec,
            );
            self.pure_i64_batch_inputs = flat_inputs;
            let values = batch_result?;
            return Ok(runtime_sequence_dense_i64(values));
        }
        items
            .iter()
            .map(|item| self.evaluate_expr_with_backend(item, pure_backend))
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_from_literal_values)
    }

    fn evaluate_range_expr(
        &mut self,
        start: Option<&RuntimeExpr>,
        end: Option<&RuntimeExpr>,
        inclusive: bool,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let start = start
            .map(|expr| self.evaluate_expr_with_backend(expr, pure_backend))
            .transpose()?;
        let end = end
            .map(|expr| self.evaluate_expr_with_backend(expr, pure_backend))
            .transpose()?;
        crate::value::RuntimeRange::new(start, end, inclusive).map(RuntimeValue::Range)
    }

    fn evaluate_repeat_seq_expr(
        &mut self,
        value: &RuntimeExpr,
        len: usize,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let RuntimeExprKind::Value(value) = value.kind() {
            return Ok(runtime_sequence_repeat_value(value, len));
        }
        (0..len)
            .map(|_| self.evaluate_expr_with_backend(value, pure_backend))
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_values)
    }

    fn bracket_seq_i64_batch_shape(
        &self,
        items: &[RuntimeExpr],
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let (first_helper, first_args) = match items.first()?.kind() {
            RuntimeExprKind::PureCall { helper, args } => (*helper, args),
            _ => return None,
        };
        if first_helper.0 >= self.plan.pure_helpers.len() || first_args.len() > RuntimeI64Args::MAX
        {
            return None;
        }
        if !self
            .pure_helper_i64_call_shapes
            .get(first_helper.0)
            .copied()
            .unwrap_or(false)
        {
            return None;
        }
        let arity = first_args.len();
        items
            .iter()
            .all(|item| match item.kind() {
                RuntimeExprKind::PureCall { helper, args } => {
                    *helper == first_helper
                        && args.len() == arity
                        && args
                            .iter()
                            .all(|arg| arg.mode() == RuntimeCallArgumentMode::Value)
                }
                _ => false,
            })
            .then_some((first_helper, arity))
    }

    fn collect_i64_pure_batch_inputs(
        &mut self,
        items: &[RuntimeExpr],
        arity: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<(), RuntimeEvalError> {
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items {
            let RuntimeExprKind::PureCall { args, .. } = item.kind() else {
                unreachable!("i64 pure batch shape checked before row collection");
            };
            for arg in args.iter().take(arity) {
                flat_inputs.push(self.evaluate_i64_arg_with_backend(arg.value(), pure_backend)?);
            }
        }
        Ok(())
    }

    fn call_i64_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[i64],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[i64]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_i64_batch_outputs);
        out.resize(row_count, 0);
        let helper = crate::pure::RuntimePureHelperRef::resolve(&self.plan, helper_id)?;
        let batch_result = pure_backend.call_i64_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_i64_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_i64_batch_outputs = out;
        Ok(result)
    }

    fn evaluate_nominal_record_expr(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        record: &RuntimeNominalRecordExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = std::sync::Arc::clone(&self.plan);
        let declaration = plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::UnknownPlanType(ty))?;
        let RuntimePlanTypeProjection::Nominal {
            nominal, layout, ..
        } = declaration.projection()
        else {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        };
        let domain = plan
            .nominal_record_domains()
            .get(ty)
            .ok_or(RuntimeEvalError::MissingNominalRecordDomain(ty))?;
        let mut fields = std::iter::repeat_with(|| None)
            .take(domain.fields().len())
            .collect::<Vec<_>>();
        for initializer in record.initializers() {
            let value = self.evaluate_expr_with_backend(initializer.value(), pure_backend)?;
            let ordinal = usize::try_from(initializer.field().zero_based())
                .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))?;
            let Some(field) = domain.fields().get(ordinal) else {
                return Err(RuntimeEvalError::InvalidExpressionType(ty));
            };
            if !plan.value_matches_type(field.ty(), &value)? {
                return Err(RuntimeEvalError::InvalidExpressionType(
                    initializer.value().ty(),
                ));
            }
            fields[ordinal] = Some(value);
        }
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(ordinal, field)| {
                let field_id =
                    crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                        .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))?;
                field.ok_or(RuntimeEvalError::MissingRecordInitializer {
                    ty,
                    field: field_id,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::NominalRecord(
            crate::value::RuntimeNominalRecordValue::new(
                nominal.clone(),
                declaration.semantic_identity(),
                *layout,
                fields,
            ),
        ))
    }

    fn evaluate_variant_expr(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        ordinal: u32,
        payload: Option<&RuntimeExpr>,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = std::sync::Arc::clone(&self.plan);
        let case = plan.variant_case(ty, ordinal)?;
        let payload = payload
            .map(|expr| self.evaluate_expr_with_backend(expr, pure_backend))
            .transpose()?;
        match (case.payload(), payload.as_ref()) {
            (Some(expected), Some(value)) if plan.value_matches_type(expected, value)? => {}
            (None, None) => {}
            _ => return Err(RuntimeEvalError::InvalidExpressionType(ty)),
        }
        Ok(RuntimeValue::Variant {
            owner: case.owner().clone(),
            ordinal,
            name: case.name().to_owned(),
            payload: payload.map(Box::new),
        })
    }

    fn evaluate_reduction_unchanged(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        state: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = std::sync::Arc::clone(&self.plan);
        let declaration = plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::UnknownPlanType(ty))?;
        let RuntimePlanTypeProjection::Opaque {
            producer,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: crate::value::RuntimeOpaqueValueClass::Plain,
            persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments,
        } = declaration.projection()
        else {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        };
        let state_ty = match plan
            .type_table()
            .get(state.ty())
            .map(RuntimePlanTypeDeclaration::projection)
        {
            Some(RuntimePlanTypeProjection::Reference(inner)) => *inner,
            _ => state.ty(),
        };
        if arguments.as_ref() != [state_ty] {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        }
        let producer = producer.clone();
        let semantic_identity = declaration.semantic_identity();
        let state = self.evaluate_expr_with_backend(state, pure_backend)?;
        if !plan.value_matches_type(state_ty, &state)? {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        }
        RuntimeReductionValue::try_unchanged(
            RuntimeOpaqueTypeOwner::exact(producer, semantic_identity),
            state,
        )
        .map(RuntimeValue::Reduction)
        .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))
    }

    fn evaluate_field_expr(
        &mut self,
        target: &RuntimeExpr,
        field: &RuntimeFieldProjection,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(target, pure_backend)?;
        match (field, value) {
            (RuntimeFieldProjection::Nominal(field), RuntimeValue::NominalRecord(record)) => record
                .field(*field)
                .cloned()
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.zero_based().to_string(),
                    value: "nominal record".to_owned(),
                }),
            (
                RuntimeFieldProjection::OpaqueRecord { owner, field },
                RuntimeValue::Opaque(value),
            ) if owner.accepts_opaque_value(&value) => {
                let RuntimeValue::Tuple(fields) = value.payload() else {
                    return Err(RuntimeEvalError::MissingField {
                        field: field.zero_based().to_string(),
                        value: "opaque record payload".to_owned(),
                    });
                };
                fields
                    .get(field.zero_based() as usize)
                    .cloned()
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: field.zero_based().to_string(),
                        value: "opaque record payload".to_owned(),
                    })
            }
            (RuntimeFieldProjection::EntityReference(field), RuntimeValue::EntityRef(id)) => {
                Ok(RuntimeValue::String(id.field_value(*field)))
            }
            (RuntimeFieldProjection::Agent(field), RuntimeValue::Agent(value)) => value
                .project_typed_field(*field)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.as_label().to_owned(),
                    value: value.label().to_owned(),
                }),
            (RuntimeFieldProjection::Agent(field), RuntimeValue::Record(fields))
                if field.permits_protocol_record() =>
            {
                fields
                    .iter()
                    .find(|entry| entry.name() == field.as_label())
                    .map(|entry| entry.value().clone())
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: field.as_label().to_owned(),
                        value: "Agent protocol record".to_owned(),
                    })
            }
            (RuntimeFieldProjection::Progress(field), RuntimeValue::Progress(progress)) => {
                Ok(match field {
                    crate::value::RuntimeProgressField::Ratio => {
                        RuntimeValue::F32(progress.ratio())
                    }
                    crate::value::RuntimeProgressField::Label => progress
                        .label()
                        .map_or_else(RuntimeValue::option_none, |label| {
                            RuntimeValue::option_some(RuntimeValue::String(label.to_owned()))
                        }),
                })
            }
            (field, value) => Err(RuntimeEvalError::MissingField {
                field: field.label(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_project_tuple_expr(
        &mut self,
        target: &RuntimeExpr,
        ordinal: usize,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(target, pure_backend)?;
        match value {
            RuntimeValue::Tuple(items) => {
                items
                    .into_iter()
                    .nth(ordinal)
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: ordinal.to_string(),
                        value: "tuple".to_owned(),
                    })
            }
            RuntimeValue::Seq(RuntimeSeq::TupleColumns(columns)) => columns
                .column(ordinal)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: ordinal.to_string(),
                    value: "tuple sequence".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: ordinal.to_string(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_project_record_expr(
        &mut self,
        target: &RuntimeExpr,
        ordinal: usize,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(target, pure_backend)?;
        match value {
            RuntimeValue::Record(fields) => fields.into_iter().nth(ordinal).map_or_else(
                || {
                    Err(RuntimeEvalError::MissingField {
                        field: ordinal.to_string(),
                        value: "record".to_owned(),
                    })
                },
                |field| Ok(field.into_value()),
            ),
            RuntimeValue::Seq(RuntimeSeq::RecordColumns(records)) => records
                .field_by_ordinal(ordinal)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: ordinal.to_string(),
                    value: "record sequence".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: ordinal.to_string(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_let_expr(
        &mut self,
        binding: RuntimeLocalDeclarationId,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        self.fiber.env.push_scope_with_capacity(1);
        self.fiber.env.set(binding, value);
        let result = self.evaluate_expr_with_backend(body, pure_backend);
        self.fiber.env.pop_scope();
        result
    }

    fn evaluate_assign_expr(
        &mut self,
        place: &crate::value::RuntimeAssignment,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        self.fiber
            .env
            .assign_place(place, value)
            .map_err(|error| error.into_parts().0)?;
        self.evaluate_expr_with_backend(body, pure_backend)
    }

    fn evaluate_if_expr(
        &mut self,
        condition: &RuntimeExpr,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if self.evaluate_bool_with_backend(condition, pure_backend)? {
            self.evaluate_expr_with_backend(then_expr, pure_backend)
        } else {
            self.evaluate_expr_with_backend(else_expr, pure_backend)
        }
    }

    pub(super) fn evaluate_if_let_expr(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        if !crate::pattern::inspect_runtime_pattern_owned(&self.plan, pattern, &value)? {
            return self.evaluate_expr_with_backend(else_expr, pure_backend);
        }
        let guard_matched = if let Some(guard) = guard {
            let projected = crate::pattern::prepare_runtime_pattern_guard_bindings(
                &self.plan, pattern, &value, guard,
            )?;
            self.with_temp_bindings(projected, |this| {
                this.evaluate_bool_with_backend(guard, pure_backend)
            })?
        } else {
            true
        };
        if guard_matched {
            let bindings = crate::pattern::match_runtime_pattern_owned(&self.plan, pattern, value)?
                .expect("checked owned pattern remains matched");
            self.with_temp_bindings(bindings, |this| {
                this.evaluate_expr_with_backend(then_expr, pure_backend)
            })
        } else {
            self.evaluate_expr_with_backend(else_expr, pure_backend)
        }
    }

    pub(super) fn evaluate_match_expr(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeExprMatchArm],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(scrutinee, pure_backend)?;
        for arm in arms {
            if !crate::pattern::inspect_runtime_pattern_owned(&self.plan, arm.pattern(), &value)? {
                continue;
            }
            if let Some(guard) = arm.guard() {
                let projected = crate::pattern::prepare_runtime_pattern_guard_bindings(
                    &self.plan,
                    arm.pattern(),
                    &value,
                    guard,
                )?;
                if !self.with_temp_bindings(projected, |this| {
                    this.evaluate_bool_with_backend(guard, pure_backend)
                })? {
                    continue;
                }
            }
            let bindings =
                crate::pattern::match_runtime_pattern_owned(&self.plan, arm.pattern(), value)?
                    .expect("checked owned pattern remains matched");
            return self.with_temp_bindings(bindings, |this| {
                this.evaluate_expr_with_backend(arm.value(), pure_backend)
            });
        }
        Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
            &value,
        )))
    }

    pub(super) fn evaluate_bool_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_expr_with_backend(expr, pure_backend)? {
            RuntimeValue::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(runtime_value_label(&value))),
        }
    }

    pub(super) fn with_temp_bindings<I, T>(
        &mut self,
        bindings: I,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T
    where
        I: IntoIterator<Item = RuntimeLocalBinding>,
        I::IntoIter: ExactSizeIterator,
    {
        let bindings = bindings.into_iter();
        self.fiber.env.push_scope_with_capacity(bindings.len());
        self.fiber.env.bind_all(bindings);
        let result = f(self);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn with_temp_binding_ref<T>(
        &mut self,
        local: RuntimeLocalDeclarationId,
        value: &RuntimeValue,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.fiber.env.push_scope_with_capacity(1);
        self.fiber.env.set_ref(local, value);
        let result = f(self);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn evaluate_entity_target(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<FlowRuntimeId, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::EntityRef(target) => {
                let target = target.runtime_label();
                self.plan
                    .resolve_flow_target_value(&target)
                    .map_err(|error| RuntimeEvalError::InvalidEntityTarget {
                        target,
                        reason: error.to_string(),
                    })
            }
            RuntimeValue::String(target) => {
                self.plan
                    .resolve_flow_target_value(&target)
                    .map_err(|error| RuntimeEvalError::InvalidEntityTarget {
                        target,
                        reason: error.to_string(),
                    })
            }
            value => Err(RuntimeEvalError::ExpectedEntityRef(runtime_value_label(
                &value,
            ))),
        }
    }

    pub(super) fn try_bind_pattern_owned(
        &mut self,
        pattern: &RuntimePattern,
        value: RuntimeValue,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        if !crate::pattern::inspect_runtime_pattern_owned(&self.plan, pattern, &value)? {
            return Ok(Some(value));
        }
        let Some(bindings) =
            crate::pattern::match_runtime_pattern_owned(&self.plan, pattern, value)?
        else {
            unreachable!("borrowed pattern preflight sealed the owning match")
        };
        self.fiber.env.bind_all(bindings);
        Ok(None)
    }

    pub(super) fn fail_eval(
        &mut self,
        error: impl std::fmt::Display,
        output: &mut RuntimeStepOutput,
    ) {
        let message = error.to_string();
        self.fiber.status = FlowFiberStatus::Failed(message.clone());
        output.diagnostics.push(RuntimeDiagnostic::new(message));
    }
}

pub(super) fn pure_helper_has_i64_call_shape(helper: &crate::plan::RuntimePureHelper) -> bool {
    helper.scalar_eval_supported
        && helper.output_type == RuntimePureOutputType::I64
        && pure_helper_has_only_inputs(helper, RuntimePureInputType::I64)
}

fn pure_helper_has_only_inputs(
    helper: &crate::plan::RuntimePureHelper,
    expected: RuntimePureInputType,
) -> bool {
    helper.input_locals.len() == helper.input_types.len()
        && helper.input_types.iter().all(|ty| *ty == expected)
}

fn spread_runtime_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
    match runtime_value_into_sequence_values(value) {
        Ok(items) => Ok(items),
        Err(value) => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
    }
}

pub(crate) fn evaluate_runtime_call(
    callee: &RuntimeCallTarget,
    mut args: Vec<RuntimeValue>,
    external_context: &RuntimeExternalCallContext,
    pure_backend: &mut impl RuntimeCallBackend,
) -> Result<RuntimeValue, RuntimeEvalError> {
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Some(value) = evaluate_std_float_intrinsic(intrinsic, &args)?
    {
        return Ok(value);
    }
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Some(value) = evaluate_string_intrinsic(intrinsic, &args)?
    {
        return Ok(value);
    }
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Some(value) = evaluate_capacity_intrinsic(intrinsic, &args)?
    {
        return Ok(value);
    }
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Some(value) = evaluate_index_intrinsic(intrinsic, &args)?
    {
        return Ok(value);
    }
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Some(value) = evaluate_core_iterator_intrinsic(intrinsic, &mut args)
    {
        return value;
    }
    evaluate_runtime_call_after_intrinsics(callee, &args, external_context, pure_backend)
}

fn evaluate_runtime_call_after_intrinsics(
    callee: &RuntimeCallTarget,
    args: &[RuntimeValue],
    external_context: &RuntimeExternalCallContext,
    pure_backend: &mut impl RuntimeCallBackend,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match (callee.as_intrinsic(), args) {
        (Some(RuntimeIntrinsic::Add), [RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)]) => {
            evaluate_binary(
                RuntimeValue::Int(*lhs),
                RuntimeBinaryOp::Add,
                RuntimeValue::Int(*rhs),
            )
        }
        (Some(RuntimeIntrinsic::CoreRange), _) => evaluate_core_range_intrinsic(args),
        (
            Some(RuntimeIntrinsic::MathMatmulF32),
            [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
        ) => pure_backend
            .call_math_matmul_f32(lhs, rhs)
            .map(RuntimeValue::matrix_f32),
        (
            Some(RuntimeIntrinsic::MathMatrixAddF32),
            [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
        ) => pure_backend
            .call_math_matrix_add_f32(lhs, rhs)
            .map(RuntimeValue::matrix_f32),
        (
            Some(RuntimeIntrinsic::MathTensorAddF32),
            [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
        ) => pure_backend
            .call_math_tensor_add_f32(lhs, rhs)
            .map(RuntimeValue::tensor_f32),
        (
            Some(RuntimeIntrinsic::MathMatmulF64),
            [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
        ) => pure_backend
            .call_math_matmul_f64(lhs, rhs)
            .map(RuntimeValue::matrix_f64),
        (
            Some(RuntimeIntrinsic::MathMatrixAddF64),
            [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
        ) => pure_backend
            .call_math_matrix_add_f64(lhs, rhs)
            .map(RuntimeValue::matrix_f64),
        (
            Some(RuntimeIntrinsic::MathTensorAddF64),
            [RuntimeValue::TensorF64(lhs), RuntimeValue::TensorF64(rhs)],
        ) => pure_backend
            .call_math_tensor_add_f64(lhs, rhs)
            .map(RuntimeValue::tensor_f64),
        _ => pure_backend
            .call_external(external_context, callee, args)
            .unwrap_or_else(|| {
                Err(RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: "no runtime backend accepted this exact call".to_owned(),
                })
            }),
    }
}

#[cfg(test)]
mod opaque_record_projection_tests {
    use super::*;
    use crate::pattern::{RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId};
    use crate::plan::{
        RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    };
    use crate::value::{
        RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeRecordFieldId,
    };

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn producer(label: &str) -> RuntimeOpaqueTypeProducerId {
        RuntimeOpaqueTypeProducerId::try_new(label).expect("test opaque producer")
    }

    fn plan_and_owner() -> (RuntimePlan, RuntimeOpaqueTypeOwner) {
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            producer("fixture.dialogue-view"),
            identity(111),
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        );
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(
                        owner.semantic_identity(),
                        RuntimePlanTypeProjection::Opaque {
                            producer: owner.producer().clone(),
                            admission: owner.admission(),
                            value_class: owner.value_class(),
                            persistence: owner.persistence(),
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(identity(112), RuntimePlanTypeProjection::String),
                ],
                [],
            )
            .expect("test type graph");
        (builder.finish().expect("test runtime plan"), owner)
    }

    fn expression(
        plan: &RuntimePlan,
        expected: &RuntimeOpaqueTypeOwner,
        actual: &RuntimeOpaqueTypeOwner,
    ) -> RuntimeExpr {
        let owner_ty = plan
            .type_table()
            .id_for_semantic(expected.semantic_identity())
            .expect("opaque owner type");
        let field_ty = plan
            .type_table()
            .id_for_semantic(identity(112))
            .expect("field type");
        let target = RuntimeExpr::from_admitted_parts(
            owner_ty,
            RuntimeExprKind::Value(
                actual
                    .try_wrap(RuntimeValue::Tuple(vec![RuntimeValue::String(
                        "accepted".to_owned(),
                    )]))
                    .expect("exact tamper fixture"),
            ),
        );
        RuntimeExpr::from_admitted_parts(
            field_ty,
            RuntimeExprKind::Field {
                target: Box::new(target),
                field: RuntimeFieldProjection::OpaqueRecord {
                    owner: expected.clone(),
                    field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0)
                        .expect("first field"),
                },
            },
        )
    }

    #[test]
    fn engine_field_projection_rejects_each_tampered_opaque_owner_dimension() {
        let (plan, expected) = plan_and_owner();
        let exact = expression(&plan, &expected, &expected);
        let tampered = [
            RuntimeOpaqueTypeOwner::exact_with(
                producer("fixture.other-dialogue-view"),
                expected.semantic_identity(),
                expected.value_class(),
                expected.persistence(),
            ),
            RuntimeOpaqueTypeOwner::exact_with(
                expected.producer().clone(),
                expected.semantic_identity(),
                RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor),
                expected.persistence(),
            ),
            RuntimeOpaqueTypeOwner::exact_with(
                expected.producer().clone(),
                expected.semantic_identity(),
                expected.value_class(),
                RuntimeOpaquePersistence::SnapshotOnly,
            ),
        ]
        .map(|actual| expression(&plan, &expected, &actual));
        let mut engine = Engine::new(plan);
        assert_eq!(
            engine.evaluate_expr(&exact).expect("exact opaque owner"),
            RuntimeValue::String("accepted".to_owned())
        );
        for (index, expression) in tampered.into_iter().enumerate() {
            let result = engine.evaluate_expr(&expression);
            if index == 1 {
                assert!(matches!(result, Err(RuntimeEvalError::AffineLiteralCopy)));
            } else {
                assert!(matches!(result, Err(RuntimeEvalError::MissingField { .. })));
            }
        }
    }
}
