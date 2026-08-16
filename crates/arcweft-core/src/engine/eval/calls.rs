use std::sync::Arc;

use super::{
    Engine, RuntimeCallBackend, RuntimeCallTarget, RuntimeEvalError, RuntimeExpr, RuntimeValue,
    evaluate_runtime_call, runtime_sequence_from_literal_values,
    runtime_value_into_sequence_values, runtime_value_label, sum_i64_sequence_ref,
};
use crate::plan::{RuntimeReceiverMode, RuntimeTraitMethodId};
use crate::pure::RuntimeFixedArgs;
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::{
    RuntimeCallArgument, RuntimeCallArgumentMode, RuntimeExprKind, RuntimeIterator, RuntimeUInt,
};

pub(crate) struct TraitMethodCallOutcome {
    pub value: RuntimeValue,
    pub updated_receiver: Option<RuntimeValue>,
}

impl Engine {
    pub(super) fn evaluate_call_expr(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let args = self.evaluate_call_args(args, pure_backend)?;
        Ok(evaluate_runtime_call(callee, &args, pure_backend))
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

    pub(super) fn evaluate_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: RuntimeLocalDeclarationId,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let iterator =
            RuntimeIterator::from_value(self.evaluate_expr_with_backend(source, pure_backend)?)
                .map_err(|value| {
                    RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value))
                })?;
        let mut mapped = Vec::new();
        for item in iterator {
            mapped.push(self.with_temp_binding_ref(param, &item, |this| {
                this.evaluate_expr_with_backend(body, pure_backend)
            })?);
        }
        Ok(runtime_sequence_from_literal_values(mapped))
    }

    pub(super) fn evaluate_sum_expr(
        &mut self,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Some(sum) = self.evaluate_u32_map_sum(source, pure_backend)? {
            return Ok(RuntimeValue::i64(sum));
        }
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

    fn evaluate_u32_map_sum(
        &mut self,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let RuntimeExprKind::Map {
            source,
            param,
            body,
        } = source.kind()
        else {
            return Ok(None);
        };
        let Some((helper_id, arity)) = self.map_u32_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u32_batch_inputs);
        let result = match self.collect_u32_map_batch_inputs(
            source,
            *param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = crate::pure::RuntimePureHelperRef::resolve(&self.plan, helper_id)?;
                pure_backend
                    .call_u32_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        flat_inputs.clear();
        self.pure_u32_batch_inputs = flat_inputs;
        result
    }

    fn map_u32_batch_shape(
        &self,
        body: &RuntimeExpr,
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let RuntimeExprKind::PureCall { helper, args } = body.kind() else {
            return None;
        };
        (helper.0 < self.plan.pure_helpers().len()
            && args.len() <= RuntimeFixedArgs::<u32>::MAX
            && args
                .iter()
                .all(|argument| argument.mode() == RuntimeCallArgumentMode::Value)
            && self
                .pure_helper_u32_call_shapes
                .get(helper.0)
                .copied()
                .unwrap_or(false))
        .then_some((*helper, args.len()))
    }

    fn collect_u32_map_batch_inputs(
        &self,
        source: &RuntimeExpr,
        param: RuntimeLocalDeclarationId,
        body: &RuntimeExpr,
        arity: usize,
        flat_inputs: &mut Vec<u32>,
    ) -> Result<Option<usize>, RuntimeEvalError> {
        let RuntimeExprKind::PureCall { args, .. } = body.kind() else {
            unreachable!("u32 map batch shape checked before input collection");
        };
        let sequence = match source.kind() {
            RuntimeExprKind::Value(RuntimeValue::Seq(sequence)) => sequence,
            RuntimeExprKind::Local(local) => match self.fiber.env.get(*local) {
                Some(RuntimeValue::Seq(sequence)) => sequence,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let Some(items) = sequence.as_u32_slice() else {
            return Ok(None);
        };
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items.iter().copied() {
            for argument in args.iter().take(arity) {
                let Some(value) = self.evaluate_u32_map_argument(argument.value(), param, item)?
                else {
                    flat_inputs.clear();
                    return Ok(None);
                };
                flat_inputs.push(value);
            }
        }
        Ok(Some(items.len()))
    }

    fn evaluate_u32_map_argument(
        &self,
        expr: &RuntimeExpr,
        param: RuntimeLocalDeclarationId,
        item: u32,
    ) -> Result<Option<u32>, RuntimeEvalError> {
        match expr.kind() {
            RuntimeExprKind::Local(local) if *local == param => Ok(Some(item)),
            RuntimeExprKind::Local(local) => self
                .fiber
                .env
                .get(*local)
                .ok_or(RuntimeEvalError::UnknownLocal(*local))
                .map(runtime_value_as_u32),
            RuntimeExprKind::Value(value) => Ok(runtime_value_as_u32(value)),
            _ => Ok(None),
        }
    }

    pub(super) fn evaluate_call_args(
        &mut self,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut values = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr_with_backend(argument.value(), pure_backend)?;
            match argument.mode() {
                RuntimeCallArgumentMode::Value => values.push(value),
                RuntimeCallArgumentMode::Spread => {
                    values.extend(runtime_value_into_sequence_values(value).map_err(|value| {
                        RuntimeEvalError::InvalidSpread(runtime_value_label(&value))
                    })?);
                }
            }
        }
        Ok(values)
    }
}

fn runtime_value_as_u32(value: &RuntimeValue) -> Option<u32> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U32(value)) => Some(*value),
        _ => None,
    }
}
