use std::sync::Arc;

use super::{
    Engine, RuntimeCallBackend, RuntimeCallTarget, RuntimeEvalError, RuntimeExpr, RuntimeValue,
    evaluate_runtime_call, runtime_sequence_from_literal_values,
    runtime_value_into_sequence_values, runtime_value_label, sum_i64_sequence_ref,
};
use crate::pattern::RuntimeBuiltinVariantCaseIdentity;
use crate::plan::{RuntimePlanTypeProjection, RuntimeReceiverMode, RuntimeTraitMethodId};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::value::{
    RuntimeCallArgument, RuntimeCallArgumentMode, RuntimeCallableValue, RuntimeIterator,
    RuntimeStandardMapFamily, RuntimeStandardMapOperandOrder,
};
use crate::{
    entry::RuntimeSchemaLimits, pattern::RuntimeSemanticTypeId, pure::RuntimeExternalCallContext,
    task::RuntimeProgramOwner,
};

pub(crate) struct TraitMethodCallOutcome {
    pub value: RuntimeValue,
    pub updated_receiver: Option<RuntimeValue>,
}

impl Engine {
    pub(super) fn evaluate_standard_map_expr(
        &mut self,
        family: RuntimeStandardMapFamily,
        order: RuntimeStandardMapOperandOrder,
        mapping: &RuntimeExpr,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let (mapping, source) = match order {
            RuntimeStandardMapOperandOrder::MappingThenReceiver => (
                self.evaluate_expr_with_backend(mapping, pure_backend)?,
                self.evaluate_expr_with_backend(source, pure_backend)?,
            ),
            RuntimeStandardMapOperandOrder::ReceiverThenMapping => {
                let source = self.evaluate_expr_with_backend(source, pure_backend)?;
                let mapping = self.evaluate_expr_with_backend(mapping, pure_backend)?;
                (mapping, source)
            }
        };
        let RuntimeValue::Callable(mapping) = mapping else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &mapping,
            )));
        };
        match family {
            RuntimeStandardMapFamily::Vec
            | RuntimeStandardMapFamily::Seq
            | RuntimeStandardMapFamily::Array
            | RuntimeStandardMapFamily::Slice => {
                self.evaluate_standard_sequence_map(&mapping, source, pure_backend)
            }
            RuntimeStandardMapFamily::Option => {
                let (case, payload) = source
                    .try_into_builtin_variant_case()
                    .map_err(|_| RuntimeEvalError::InvalidStandardMapSource { family })?;
                match (case, payload) {
                    (RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value)) => self
                        .apply_runtime_function(&mapping, &[value], pure_backend)
                        .map(RuntimeValue::option_some),
                    (RuntimeBuiltinVariantCaseIdentity::OptionNone, None) => {
                        Ok(RuntimeValue::option_none())
                    }
                    _ => Err(RuntimeEvalError::InvalidStandardMapSource { family }),
                }
            }
            RuntimeStandardMapFamily::Result => {
                let (case, payload) = source
                    .try_into_builtin_variant_case()
                    .map_err(|_| RuntimeEvalError::InvalidStandardMapSource { family })?;
                match (case, payload) {
                    (RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(value)) => self
                        .apply_runtime_function(&mapping, &[value], pure_backend)
                        .map(RuntimeValue::result_ok),
                    (RuntimeBuiltinVariantCaseIdentity::ResultErr, Some(error)) => {
                        Ok(RuntimeValue::result_err(error))
                    }
                    _ => Err(RuntimeEvalError::InvalidStandardMapSource { family }),
                }
            }
        }
    }

    fn evaluate_standard_sequence_map(
        &mut self,
        mapping: &RuntimeCallableValue,
        source: RuntimeValue,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let iterator = RuntimeIterator::from_value(source)
            .map_err(|value| RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value)))?;
        let mut mapped = Vec::new();
        for item in iterator {
            mapped.push(self.apply_runtime_function(mapping, &[item], pure_backend)?);
        }
        Ok(runtime_sequence_from_literal_values(mapped))
    }

    pub(super) fn evaluate_call_expr(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeCallArgument],
        result_type: RuntimePlanTypeId,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let (args, context) = if callee.as_intrinsic().is_some() {
            (
                self.evaluate_call_args(args, pure_backend)?,
                RuntimeExternalCallContext::unbound(),
            )
        } else {
            let (values, argument_types) =
                self.evaluate_external_call_args(callee, args, pure_backend)?;
            let result_type = self.external_call_semantic_type(callee, result_type)?;
            let context = RuntimeExternalCallContext::for_program(
                RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
                argument_types,
                result_type,
                RuntimeSchemaLimits::engine_default(),
            )
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: callee.as_label().to_owned(),
                reason: format!(
                    "external call signature is absent from its selected plan: {error}"
                ),
            })?;
            (values, context)
        };
        Ok(evaluate_runtime_call(callee, &args, &context, pure_backend))
    }

    fn evaluate_external_call_args(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(Vec<RuntimeValue>, Box<[RuntimeSemanticTypeId]>), RuntimeEvalError> {
        let mut materialized = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr_with_backend(argument.value(), pure_backend)?;
            let argument_type = argument.value().ty();
            let (values, semantic_types) = match argument.mode() {
                RuntimeCallArgumentMode::Value => (
                    vec![value],
                    vec![self.external_call_semantic_type(callee, argument_type)?],
                ),
                RuntimeCallArgumentMode::Spread => {
                    let values = runtime_value_into_sequence_values(value).map_err(|value| {
                        RuntimeEvalError::InvalidSpread(runtime_value_label(&value))
                    })?;
                    let semantic_types =
                        self.external_spread_semantic_types(callee, argument_type, values.len())?;
                    if values.len() != semantic_types.len() {
                        return Err(RuntimeEvalError::UnsupportedPure {
                            name: callee.as_label().to_owned(),
                            reason: format!(
                                "external spread produced {} value(s) for {} semantic argument type(s)",
                                values.len(),
                                semantic_types.len()
                            ),
                        });
                    }
                    (values, semantic_types)
                }
            };
            materialized.push((argument.abi_position(), values, semantic_types));
        }
        materialized.sort_by_key(|(position, _, _)| *position);
        let value_count = materialized.iter().map(|(_, values, _)| values.len()).sum();
        let mut values = Vec::with_capacity(value_count);
        let mut semantic_types = Vec::with_capacity(value_count);
        for (_, source_values, source_types) in materialized {
            if source_values.len() != source_types.len() {
                return Err(RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: format!(
                        "external call has {} value(s) for {} semantic argument type(s)",
                        source_values.len(),
                        source_types.len()
                    ),
                });
            }
            values.extend(source_values);
            semantic_types.extend(source_types);
        }
        if values.len() != semantic_types.len() {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: callee.as_label().to_owned(),
                reason: format!(
                    "external call has {} value(s) for {} semantic argument type(s)",
                    values.len(),
                    semantic_types.len()
                ),
            });
        }
        Ok((values, semantic_types.into_boxed_slice()))
    }

    fn external_call_semantic_type(
        &self,
        callee: &RuntimeCallTarget,
        ty: RuntimePlanTypeId,
    ) -> Result<RuntimeSemanticTypeId, RuntimeEvalError> {
        self.plan
            .type_table()
            .get(ty)
            .map(|declaration| declaration.semantic_identity())
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: callee.as_label().to_owned(),
                reason: format!(
                    "external call references plan type {ty} that is absent from the selected plan"
                ),
            })
    }

    fn external_spread_semantic_types(
        &self,
        callee: &RuntimeCallTarget,
        ty: RuntimePlanTypeId,
        value_count: usize,
    ) -> Result<Vec<RuntimeSemanticTypeId>, RuntimeEvalError> {
        let declaration = self
            .plan
            .type_table()
            .get(ty)
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: callee.as_label().to_owned(),
                reason: format!(
                    "external spread references plan type {ty} that is absent from the selected plan"
                ),
            })?;
        let child_types = match declaration.projection() {
            RuntimePlanTypeProjection::Sequence { item, .. }
            | RuntimePlanTypeProjection::Array { item, .. } => vec![*item; value_count],
            RuntimePlanTypeProjection::Tuple(items) if items.len() == value_count => items.to_vec(),
            RuntimePlanTypeProjection::Tuple(items) => {
                return Err(RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: format!(
                        "external spread produced {value_count} value(s) for a tuple with {} field type(s)",
                        items.len()
                    ),
                });
            }
            _ => {
                return Err(RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: format!(
                        "external spread type {ty} has no sequence, array, or tuple element types"
                    ),
                });
            }
        };
        child_types
            .into_iter()
            .map(|child| self.external_call_semantic_type(callee, child))
            .collect()
    }

    pub(super) fn evaluate_pure_call_expr(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let args = self.evaluate_call_args(args, pure_backend)?;
        let helper = crate::pure::RuntimePureHelperRef::resolve(&self.plan, helper_id)?;
        pure_backend.call_values(helper, &args)
    }

    pub(crate) fn evaluate_trait_method_call(
        &mut self,
        callable: RuntimeTraitMethodId,
        receiver_mode: RuntimeReceiverMode,
        receiver: &RuntimeExpr,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<TraitMethodCallOutcome, RuntimeEvalError> {
        let receiver_value = self.evaluate_expr_with_backend(receiver, pure_backend)?;
        let arg_values = self.evaluate_call_args(args, pure_backend)?;
        self.evaluate_trait_method_values(
            callable,
            receiver_mode,
            receiver_value,
            arg_values,
            pure_backend,
        )
    }

    pub(crate) fn evaluate_trait_method_values(
        &mut self,
        callable: RuntimeTraitMethodId,
        receiver_mode: RuntimeReceiverMode,
        receiver_value: RuntimeValue,
        arg_values: Vec<RuntimeValue>,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<TraitMethodCallOutcome, RuntimeEvalError> {
        let plan = Arc::clone(&self.plan);
        let method = plan
            .trait_methods
            .get(callable.0)
            .ok_or(RuntimeEvalError::UnknownTraitMethod(callable.0))?;
        let expected_args = method.input_locals.len().saturating_sub(1);
        if expected_args != arg_values.len() {
            return Err(RuntimeEvalError::TraitMethodArgumentCount {
                method: method.identity.method_name.clone(),
                expected: expected_args,
                found: arg_values.len(),
            });
        }

        let receiver_local = method.input_locals.first().copied().ok_or_else(|| {
            RuntimeEvalError::InvalidTraitReceiverUpdate {
                method: method.identity.method_name.clone(),
                receiver: method.identity.self_type.clone(),
            }
        })?;
        self.validate_local_value(receiver_local, &receiver_value)?;
        for (&local, value) in method.input_locals.iter().skip(1).zip(&arg_values) {
            self.validate_local_value(local, value)?;
        }
        self.fiber
            .env
            .push_scope_with_capacity(method.input_locals.len());
        self.fiber.env.set(receiver_local, receiver_value);
        for (&local, value) in method.input_locals.iter().skip(1).zip(arg_values) {
            self.fiber.env.set(local, value);
        }
        let value = self.evaluate_expr_with_backend(&method.body, pure_backend);
        let updated_receiver = if receiver_mode == RuntimeReceiverMode::MutRef {
            method
                .input_locals
                .first()
                .and_then(|&local| self.fiber.env.get_cloned(local))
        } else {
            None
        };
        self.fiber.env.pop_scope();

        let value = value?;
        if receiver_mode == RuntimeReceiverMode::MutRef && updated_receiver.is_none() {
            return Err(RuntimeEvalError::InvalidTraitReceiverUpdate {
                method: method.identity.method_name.clone(),
                receiver: method.identity.self_type.clone(),
            });
        }
        Ok(TraitMethodCallOutcome {
            value,
            updated_receiver,
        })
    }

    fn validate_local_value(
        &self,
        local: RuntimeLocalDeclarationId,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeEvalError> {
        let declaration = self
            .plan
            .local_declarations()
            .get(local)
            .ok_or(RuntimeEvalError::UnknownLocal(local))?;
        if !self.plan.value_matches_type(declaration.ty(), value)? {
            return Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()));
        }
        Ok(())
    }

    pub(super) fn evaluate_i64_arg_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<i64, RuntimeEvalError> {
        match self.evaluate_expr_with_backend(expr, pure_backend)? {
            RuntimeValue::Int(value) => value.exact_i64().ok_or_else(|| {
                RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
            }),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
        }
    }

    pub(super) fn evaluate_sum_expr(
        &mut self,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(source, pure_backend)?;
        if let RuntimeValue::Seq(sequence) = &value
            && let Some(sum) = sequence.sum_as_i64()
        {
            return Ok(RuntimeValue::i64(sum));
        }
        let iterator = RuntimeIterator::from_value(value)
            .map_err(|value| RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value)))?;
        let items = iterator.collect::<Vec<_>>();
        sum_i64_sequence_ref(&items).map(RuntimeValue::i64)
    }

    pub(super) fn evaluate_call_args(
        &mut self,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut materialized = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr_with_backend(argument.value(), pure_backend)?;
            let values = match argument.mode() {
                RuntimeCallArgumentMode::Value => vec![value],
                RuntimeCallArgumentMode::Spread => runtime_value_into_sequence_values(value)
                    .map_err(|value| {
                        RuntimeEvalError::InvalidSpread(runtime_value_label(&value))
                    })?,
            };
            materialized.push((argument.abi_position(), values));
        }
        materialized.sort_by_key(|(position, _)| *position);
        let mut values = Vec::new();
        for (_, materialized) in materialized {
            values.extend(materialized);
        }
        Ok(values)
    }
}
