use super::{
    Engine, RuntimeCallBackend, RuntimeCallTarget, RuntimeEvalError, RuntimeExactInteger,
    RuntimeExpr, RuntimeFixedArgs, RuntimeFloat32Args, RuntimeFloat64Args, RuntimeI32Args,
    RuntimeI64Args, RuntimeISizeValue, RuntimePureCallBackend, RuntimePureScalarInteger,
    RuntimeSeq, RuntimeUSizeValue, RuntimeValue, evaluate_runtime_call,
    evaluate_runtime_method_call, pure_helper_has_exact_int_call_shape, runtime_sequence_dense_f32,
    runtime_sequence_dense_f64, runtime_sequence_dense_i8, runtime_sequence_dense_i16,
    runtime_sequence_dense_i32, runtime_sequence_dense_i64, runtime_sequence_dense_i128,
    runtime_sequence_dense_u8, runtime_sequence_dense_u16, runtime_sequence_dense_u32,
    runtime_sequence_dense_u64, runtime_sequence_dense_u128, runtime_sequence_values,
    runtime_value_into_sequence_values, runtime_value_label, spread_runtime_values,
    sum_i64_sequence_ref,
};
use crate::plan::{RuntimeReceiverMode, RuntimeTraitMethodId};
use crate::value::RuntimeIterator;

pub(crate) struct TraitMethodCallOutcome {
    pub value: RuntimeValue,
    pub updated_receiver: Option<RuntimeValue>,
}

impl Engine {
    pub(super) fn evaluate_call_expr(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let args = self.evaluate_call_args(args, pure_backend)?;
        Ok(evaluate_runtime_call(callee, &args, pure_backend))
    }

    pub(super) fn evaluate_pure_call_expr(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if helper_id.0 >= self.plan.pure_helpers.len() {
            return Err(RuntimeEvalError::UnknownPureHelper(helper_id.0));
        }
        if self
            .pure_helper_i64_call_shapes
            .get(helper_id.0)
            .copied()
            .unwrap_or(false)
            && args.len() <= RuntimeI64Args::MAX
            && !args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
        {
            let mut values = [0_i64; RuntimeI64Args::MAX];
            for (index, arg) in args.iter().enumerate() {
                values[index] = self.evaluate_i64_arg_with_backend(arg, pure_backend)?;
            }
            let helper = &self.plan.pure_helpers[helper_id.0];
            if let Some(value) = pure_backend.call_i64_slice(helper, &values[..args.len()])? {
                return Ok(RuntimeValue::i64(value));
            }
        }
        if self
            .pure_helper_u32_call_shapes
            .get(helper_id.0)
            .copied()
            .unwrap_or(false)
            && args.len() <= RuntimeFixedArgs::<u32>::MAX
            && !args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
        {
            let mut values = [0_u32; RuntimeFixedArgs::<u32>::MAX];
            for (index, arg) in args.iter().enumerate() {
                values[index] =
                    self.evaluate_exact_int_arg_with_backend::<u32>(arg, pure_backend)?;
            }
            let helper = &self.plan.pure_helpers[helper_id.0];
            if let Some(value) = pure_backend.call_u32_slice(helper, &values[..args.len()])? {
                return Ok(RuntimeValue::u32(value));
            }
        }
        if let Some(value) = self.evaluate_exact_int_pure_call_any(helper_id, args, pure_backend)? {
            return Ok(value);
        }
        if self
            .pure_helper_f32_call_shapes
            .get(helper_id.0)
            .copied()
            .unwrap_or(false)
            && args.len() <= RuntimeFloat32Args::MAX
            && !args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
        {
            let mut values = [f32::from_bits(0); RuntimeFloat32Args::MAX];
            for (index, arg) in args.iter().enumerate() {
                values[index] = self.evaluate_f32_arg_with_backend(arg, pure_backend)?;
            }
            let helper = &self.plan.pure_helpers[helper_id.0];
            if let Some(value) = pure_backend.call_f32_slice(helper, &values[..args.len()])? {
                return Ok(RuntimeValue::F32(value));
            }
        }
        if self
            .pure_helper_f64_call_shapes
            .get(helper_id.0)
            .copied()
            .unwrap_or(false)
            && args.len() <= RuntimeFloat64Args::MAX
            && !args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
        {
            let mut values = [f64::from_bits(0); RuntimeFloat64Args::MAX];
            for (index, arg) in args.iter().enumerate() {
                values[index] = self.evaluate_f64_arg_with_backend(arg, pure_backend)?;
            }
            let helper = &self.plan.pure_helpers[helper_id.0];
            if let Some(value) = pure_backend.call_f64_slice(helper, &values[..args.len()])? {
                return Ok(RuntimeValue::F64(value));
            }
        }
        let args = self.evaluate_call_args(args, pure_backend)?;
        let helper = &self.plan.pure_helpers[helper_id.0];
        pure_backend.call_values(helper, &args)
    }

    pub(crate) fn evaluate_trait_method_call(
        &mut self,
        callable: RuntimeTraitMethodId,
        receiver_mode: RuntimeReceiverMode,
        receiver: &RuntimeExpr,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<TraitMethodCallOutcome, RuntimeEvalError> {
        let Some(method) = self.plan.trait_methods.get(callable.0).cloned() else {
            return Err(RuntimeEvalError::UnknownTraitMethod(callable.0));
        };
        let receiver_value = self.evaluate_expr_with_backend(receiver, pure_backend)?;
        let arg_values = self.evaluate_call_args(args, pure_backend)?;
        let expected_args = method.input_names.len().saturating_sub(1);
        if expected_args != arg_values.len() {
            return Err(RuntimeEvalError::TraitMethodArgumentCount {
                method: method.identity.method_name.clone(),
                expected: expected_args,
                found: arg_values.len(),
            });
        }

        self.fiber
            .env
            .push_scope_with_capacity(method.input_names.len());
        if let Some(receiver_name) = method.input_names.first() {
            self.fiber.env.set(receiver_name.clone(), receiver_value);
        }
        for (name, value) in method.input_names.iter().skip(1).zip(arg_values) {
            self.fiber.env.set(name.clone(), value);
        }
        let value = self.evaluate_expr_with_backend(&method.body, pure_backend);
        let updated_receiver = if receiver_mode == RuntimeReceiverMode::MutRef {
            method
                .input_names
                .first()
                .and_then(|name| self.fiber.env.get_cloned(name))
        } else {
            None
        };
        self.fiber.env.pop_scope();

        let value = value?;
        if receiver_mode == RuntimeReceiverMode::MutRef && updated_receiver.is_none() {
            return Err(RuntimeEvalError::InvalidTraitReceiverUpdate {
                method: method.identity.method_name,
                receiver: method.identity.self_type,
            });
        }
        Ok(TraitMethodCallOutcome {
            value,
            updated_receiver,
        })
    }

    fn evaluate_exact_int_pure_call_any(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        if let Some(value) = self.evaluate_dedicated_exact_int_pure_call::<i8, _, _>(
            helper_id,
            args,
            pure_backend,
            RuntimePureCallBackend::call_i8_slice,
        )? {
            return Ok(Some(value));
        }
        if let Some(value) = self.evaluate_dedicated_exact_int_pure_call::<i16, _, _>(
            helper_id,
            args,
            pure_backend,
            RuntimePureCallBackend::call_i16_slice,
        )? {
            return Ok(Some(value));
        }
        if let Some(value) = self.evaluate_dedicated_exact_int_pure_call::<i32, _, _>(
            helper_id,
            args,
            pure_backend,
            RuntimePureCallBackend::call_i32_slice,
        )? {
            return Ok(Some(value));
        }
        if let Some(value) = self.evaluate_dedicated_exact_int_pure_call::<u8, _, _>(
            helper_id,
            args,
            pure_backend,
            RuntimePureCallBackend::call_u8_slice,
        )? {
            return Ok(Some(value));
        }
        if let Some(value) = self.evaluate_dedicated_exact_int_pure_call::<u16, _, _>(
            helper_id,
            args,
            pure_backend,
            RuntimePureCallBackend::call_u16_slice,
        )? {
            return Ok(Some(value));
        }
        if let Some(value) = self.evaluate_dedicated_exact_int_pure_call::<u64, _, _>(
            helper_id,
            args,
            pure_backend,
            RuntimePureCallBackend::call_u64_slice,
        )? {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<i8>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<i16>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<i32>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<i128>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<RuntimeISizeValue>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<u8>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<u16>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<u32>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<u64>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.evaluate_exact_int_pure_call::<u128>(helper_id, args, pure_backend)?
        {
            return Ok(Some(value));
        }
        self.evaluate_exact_int_pure_call::<RuntimeUSizeValue>(helper_id, args, pure_backend)
    }

    fn evaluate_dedicated_exact_int_pure_call<T, B, F>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        args: &[RuntimeExpr],
        pure_backend: &mut B,
        call: F,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError>
    where
        T: RuntimePureScalarInteger + Default,
        B: RuntimeCallBackend,
        F: FnOnce(
            &mut B,
            &crate::plan::RuntimePureHelper,
            &[T],
        ) -> Result<Option<T>, RuntimeEvalError>,
    {
        let helper = &self.plan.pure_helpers[helper_id.0];
        if args.len() > RuntimeFixedArgs::<T>::MAX
            || args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
            || !pure_helper_has_exact_int_call_shape::<T>(helper)
        {
            return Ok(None);
        }
        let mut values = [T::default(); RuntimeI64Args::MAX];
        for (index, arg) in args.iter().enumerate() {
            values[index] = self.evaluate_exact_int_arg_with_backend::<T>(arg, pure_backend)?;
        }
        let helper = &self.plan.pure_helpers[helper_id.0];
        call(pure_backend, helper, &values[..args.len()])
            .map(|value| value.map(RuntimeExactInteger::into_runtime_value))
    }

    fn evaluate_exact_int_pure_call<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError>
    where
        T: RuntimePureScalarInteger + Default,
    {
        let helper = &self.plan.pure_helpers[helper_id.0];
        if args.len() > RuntimeFixedArgs::<T>::MAX
            || args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
            || !pure_helper_has_exact_int_call_shape::<T>(helper)
        {
            return Ok(None);
        }
        let mut values = [T::default(); RuntimeI64Args::MAX];
        for (index, arg) in args.iter().enumerate() {
            values[index] = self.evaluate_exact_int_arg_with_backend::<T>(arg, pure_backend)?;
        }
        let helper = &self.plan.pure_helpers[helper_id.0];
        pure_backend
            .call_exact_int_slice(helper, &values[..args.len()])
            .map(|value| value.map(RuntimeExactInteger::into_runtime_value))
    }

    fn evaluate_exact_int_arg_with_backend<T>(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<T, RuntimeEvalError>
    where
        T: RuntimePureScalarInteger,
    {
        match expr {
            RuntimeExpr::Value(value) => {
                T::try_from_runtime_value("exact integer pure call", value.clone())
            }
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(value) => T::try_from_runtime_value("exact integer pure call", value.clone()),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => {
                let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
                T::try_from_runtime_value("exact integer pure call", value)
            }
        }
    }

    pub(super) fn evaluate_i64_arg_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<i64, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::Int(value)) => value.exact_i64().ok_or_else(|| {
                RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(*value)))
            }),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Int(value)) => value.exact_i64().ok_or_else(|| {
                    RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(*value)))
                }),
                Some(value) => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => match self.evaluate_expr_with_backend(expr, pure_backend)? {
                RuntimeValue::Int(value) => value.exact_i64().ok_or_else(|| {
                    RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
                }),
                value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
            },
        }
    }

    fn evaluate_f32_arg_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<f32, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::F32(value)) => Ok(*value),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::F32(value)) => Ok(*value),
                Some(value) => Err(RuntimeEvalError::UnsupportedPure {
                    name: "f32 pure call".to_owned(),
                    reason: format!("expected f32 argument, got {}", runtime_value_label(value)),
                }),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => match self.evaluate_expr_with_backend(expr, pure_backend)? {
                RuntimeValue::F32(value) => Ok(value),
                value => Err(RuntimeEvalError::UnsupportedPure {
                    name: "f32 pure call".to_owned(),
                    reason: format!("expected f32 argument, got {}", runtime_value_label(&value)),
                }),
            },
        }
    }

    fn evaluate_f64_arg_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<f64, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::F64(value)) => Ok(*value),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::F64(value)) => Ok(*value),
                Some(value) => Err(RuntimeEvalError::UnsupportedPure {
                    name: "f64 pure call".to_owned(),
                    reason: format!("expected f64 argument, got {}", runtime_value_label(value)),
                }),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => match self.evaluate_expr_with_backend(expr, pure_backend)? {
                RuntimeValue::F64(value) => Ok(value),
                value => Err(RuntimeEvalError::UnsupportedPure {
                    name: "f64 pure call".to_owned(),
                    reason: format!("expected f64 argument, got {}", runtime_value_label(&value)),
                }),
            },
        }
    }

    pub(super) fn evaluate_method_call_expr(
        &mut self,
        receiver: &RuntimeExpr,
        method: &str,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let receiver = self.evaluate_expr_with_backend(receiver, pure_backend)?;
        let args = self.evaluate_call_args(args, pure_backend)?;
        Ok(evaluate_runtime_method_call(receiver, method, &args))
    }

    pub(super) fn evaluate_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Some(value) = self.evaluate_i8_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_i16_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_i128_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_isize_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_u8_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_u16_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_u32_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_u64_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_u128_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_usize_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_f32_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_f64_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_i32_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_i64_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        let iterator = match RuntimeIterator::from_value(
            self.evaluate_expr_with_backend(source, pure_backend)?,
        ) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        iterator
            .collect::<Vec<_>>()
            .into_iter()
            .map(|item| {
                self.with_temp_binding_ref(param, &item, |this| {
                    this.evaluate_expr_with_backend(body, pure_backend)
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_values)
    }

    fn evaluate_i8_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<i8>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i8_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<i8>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_i8_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_i8(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_i8_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_i16_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<i16>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i16_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<i16>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_i16_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_i16(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_i16_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_i128_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<i128>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i128_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<i128>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_i128_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_i128(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_i128_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_isize_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_isize_batch_inputs);
        let mut out = std::mem::take(&mut self.pure_isize_batch_outputs);
        let result = self.evaluate_exact_int_map_expr_with_buffers::<RuntimeISizeValue>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
            &mut out,
        );
        self.pure_isize_batch_inputs = flat_inputs;
        self.pure_isize_batch_outputs = out;
        result
    }

    fn evaluate_u8_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<u8>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u8_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u8>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_u8_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_u8(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u8_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u16_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<u16>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u16_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u16>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_u16_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_u16(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u16_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u32_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_u32_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u32_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u32>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let values = self.call_u32_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    <[u32]>::to_vec,
                )?;
                Ok(Some(runtime_sequence_dense_u32(values)))
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u32_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u64_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<u64>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u64_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u64>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_u64_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_u64(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u64_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u128_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<u128>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u128_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u128>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_u128_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_u128(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u128_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_usize_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_usize_batch_inputs);
        let mut out = std::mem::take(&mut self.pure_usize_batch_outputs);
        let result = self.evaluate_exact_int_map_expr_with_buffers::<RuntimeUSizeValue>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
            &mut out,
        );
        self.pure_usize_batch_inputs = flat_inputs;
        self.pure_usize_batch_outputs = out;
        result
    }

    fn evaluate_exact_int_map_expr_with_buffers<T>(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
        flat_inputs: &mut Vec<T>,
        out: &mut Vec<T>,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError>
    where
        T: RuntimePureScalarInteger + Default,
    {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<T>(body) else {
            return Ok(None);
        };
        let Some(row_count) = self.collect_exact_int_map_batch_inputs_from_borrowed_source(
            source,
            param,
            body,
            arity,
            flat_inputs,
        )?
        else {
            return Ok(None);
        };
        out.resize(row_count, T::default());
        let helper = &self.plan.pure_helpers[helper_id.0];
        if let Err(error) = pure_backend.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
        {
            out.clear();
            return Err(error);
        }
        let value = T::dense_sequence(out.clone());
        out.clear();
        Ok(Some(value))
    }

    fn evaluate_i32_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_i32_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i32_batch_inputs);
        let result = match self.collect_i32_map_batch_inputs_from_borrowed_source(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let values = self.call_i32_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    <[i32]>::to_vec,
                )?;
                Ok(Some(runtime_sequence_dense_i32(values)))
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_i32_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_f32_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_f32_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_f32_batch_inputs);
        let result = match self.collect_f32_map_batch_inputs_from_borrowed_source(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_f32_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_f32(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_f32_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_f64_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_f64_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_f64_batch_inputs);
        let result = match self.collect_f64_map_batch_inputs_from_borrowed_source(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => self
                .call_f64_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                    |values| runtime_sequence_dense_f64(values.to_vec()),
                )
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_f64_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_i64_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_i64_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i64_batch_inputs);
        if let Some(row_count) = self.collect_i64_map_batch_inputs_from_borrowed_source(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        )? {
            let values = self.call_i64_flat_batch_with_outputs(
                helper_id,
                &flat_inputs,
                arity,
                row_count,
                pure_backend,
                <[i64]>::to_vec,
            )?;
            self.pure_i64_batch_inputs = flat_inputs;
            return Ok(Some(runtime_sequence_dense_i64(values)));
        }
        let items = match self.evaluate_expr_with_backend(source, pure_backend) {
            Ok(value) => match runtime_value_into_sequence_values(value) {
                Ok(items) => items,
                Err(value) => {
                    self.pure_i64_batch_inputs = flat_inputs;
                    return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                        &value,
                    )));
                }
            },
            Err(error) => {
                self.pure_i64_batch_inputs = flat_inputs;
                return Err(error);
            }
        };
        let result = self
            .collect_i64_map_batch_inputs(
                &items,
                param,
                body,
                arity,
                pure_backend,
                &mut flat_inputs,
            )
            .and_then(|()| {
                self.call_i64_flat_batch_with_outputs(
                    helper_id,
                    &flat_inputs,
                    arity,
                    items.len(),
                    pure_backend,
                    <[i64]>::to_vec,
                )
            })
            .map(runtime_sequence_dense_i64)
            .map(Some);
        self.pure_i64_batch_inputs = flat_inputs;
        result
    }

    pub(super) fn evaluate_sum_expr(
        &mut self,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Some(sum) = self.evaluate_map_sum_expr(source, pure_backend)? {
            return Ok(RuntimeValue::i64(sum));
        }
        if let Some(sum) = self.evaluate_i64_bracket_seq_sum(source, pure_backend)? {
            return Ok(RuntimeValue::i64(sum));
        }
        if let RuntimeExpr::Local(name) = source
            && let Some(sum) = self.evaluate_i64_local_sequence_sum(name)?
        {
            return Ok(RuntimeValue::i64(sum));
        }
        let value = self.evaluate_expr_with_backend(source, pure_backend)?;
        if let RuntimeValue::Seq(seq) = &value
            && let Some(sum) = seq.sum_as_i64()
        {
            return Ok(RuntimeValue::i64(sum));
        }
        let iterator = match RuntimeIterator::from_value(value) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        let items = iterator.collect::<Vec<_>>();
        sum_i64_sequence_ref(&items).map(RuntimeValue::i64)
    }

    fn evaluate_map_sum_expr(
        &mut self,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let RuntimeExpr::Map {
            source: map_source,
            param,
            body,
        } = source
        else {
            return Ok(None);
        };
        if let Some(sum) = self.evaluate_i8_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_i16_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_i32_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_i128_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_isize_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_u8_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_u16_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_u32_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_u64_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_u128_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        if let Some(sum) = self.evaluate_usize_map_sum(map_source, param, body, pure_backend)? {
            return Ok(Some(sum));
        }
        self.evaluate_i64_map_sum(map_source, param, body, pure_backend)
    }

    fn evaluate_i64_local_sequence_sum(&self, name: &str) -> Result<Option<i64>, RuntimeEvalError> {
        let Some(value) = self.fiber.env.get(name) else {
            return Ok(None);
        };
        match value {
            RuntimeValue::Seq(seq) => match seq {
                RuntimeSeq::Values(items) => sum_i64_sequence_ref(items).map(Some),
                RuntimeSeq::Dense(items) => Ok(items.sum_as_i64()),
                RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => Ok(None),
            },
            RuntimeValue::Tuple(items) => sum_i64_sequence_ref(items).map(Some),
            _ => Ok(None),
        }
    }

    fn evaluate_i8_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<i8>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i8_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<i8>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_i8_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_i8_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_i16_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<i16>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i16_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<i16>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_i16_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_i16_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_i32_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_i32_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i32_batch_inputs);
        match self.collect_i32_map_batch_inputs_from_borrowed_source(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                let batch_result =
                    pure_backend.call_i32_flat_batch_sum(helper, &flat_inputs, arity, row_count);
                self.pure_i32_batch_inputs = flat_inputs;
                let sum = batch_result?;
                Ok(Some(sum))
            }
            Ok(None) => {
                self.pure_i32_batch_inputs = flat_inputs;
                Ok(None)
            }
            Err(error) => {
                self.pure_i32_batch_inputs = flat_inputs;
                Err(error)
            }
        }
    }

    fn evaluate_i128_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<i128>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i128_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<i128>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_i128_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_i128_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_isize_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_isize_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<RuntimeISizeValue>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_isize_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u8_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<u8>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u8_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u8>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_u8_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u8_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u16_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<u16>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u16_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u16>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_u16_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u16_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u32_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_u32_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u32_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u32>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_u32_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u32_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u64_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<u64>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u64_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u64>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_u64_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u64_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u128_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<u128>(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_u128_batch_inputs);
        let result = match self.collect_exact_int_map_batch_inputs_from_borrowed_source::<u128>(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_u128_flat_batch_sum(helper, &flat_inputs, arity, row_count)
                    .map(Some)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        self.pure_u128_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_usize_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_usize_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<RuntimeUSizeValue>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_usize_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_exact_int_map_sum_with_inputs<T: RuntimePureScalarInteger>(
        &self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
        flat_inputs: &mut Vec<T>,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_exact_int_batch_shape::<T>(body) else {
            return Ok(None);
        };
        match self.collect_exact_int_map_batch_inputs_from_borrowed_source(
            source,
            param,
            body,
            arity,
            flat_inputs,
        )? {
            Some(row_count) => {
                let helper = &self.plan.pure_helpers[helper_id.0];
                pure_backend
                    .call_exact_int_flat_batch_sum(helper, flat_inputs, arity, row_count)
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn evaluate_i64_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some((helper_id, arity)) = self.map_i64_batch_shape(body) else {
            return Ok(None);
        };
        let mut flat_inputs = std::mem::take(&mut self.pure_i64_batch_inputs);
        if let Some(row_count) = self.i64_map_borrowed_source_len(source)
            && self.collect_i64_repeated_map_batch_row_from_borrowed_source(
                source,
                param,
                body,
                arity,
                &mut flat_inputs,
            )?
        {
            let batch_result = self.call_i64_repeated_flat_batch_sum(
                helper_id,
                &flat_inputs,
                row_count,
                pure_backend,
            );
            self.pure_i64_batch_inputs = flat_inputs;
            let sum = batch_result?;
            return Ok(Some(sum));
        }
        match self.collect_i64_map_batch_inputs_from_borrowed_source(
            source,
            param,
            body,
            arity,
            &mut flat_inputs,
        ) {
            Ok(Some(row_count)) => {
                let batch_result = self.call_i64_flat_batch_sum(
                    helper_id,
                    &flat_inputs,
                    arity,
                    row_count,
                    pure_backend,
                );
                self.pure_i64_batch_inputs = flat_inputs;
                let sum = batch_result?;
                return Ok(Some(sum));
            }
            Ok(None) => {}
            Err(error) => {
                self.pure_i64_batch_inputs = flat_inputs;
                return Err(error);
            }
        }
        let items = match self.evaluate_expr_with_backend(source, pure_backend) {
            Ok(value) => match runtime_value_into_sequence_values(value) {
                Ok(items) => items,
                Err(value) => {
                    self.pure_i64_batch_inputs = flat_inputs;
                    return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                        &value,
                    )));
                }
            },
            Err(error) => {
                self.pure_i64_batch_inputs = flat_inputs;
                return Err(error);
            }
        };
        let RuntimeExpr::PureCall { args, .. } = body else {
            unreachable!("i64 map batch shape checked before repeated row collection");
        };
        if self.collect_i64_repeated_map_batch_row_without_scope(
            &items,
            param,
            args,
            arity,
            &mut flat_inputs,
        )? {
            let batch_result = self.call_i64_repeated_flat_batch_sum(
                helper_id,
                &flat_inputs,
                items.len(),
                pure_backend,
            );
            self.pure_i64_batch_inputs = flat_inputs;
            let sum = batch_result?;
            return Ok(Some(sum));
        }
        let collect_result = self.collect_i64_map_batch_inputs(
            &items,
            param,
            body,
            arity,
            pure_backend,
            &mut flat_inputs,
        );
        if let Err(error) = collect_result {
            self.pure_i64_batch_inputs = flat_inputs;
            return Err(error);
        }
        let batch_result =
            self.call_i64_flat_batch_sum(helper_id, &flat_inputs, arity, items.len(), pure_backend);
        self.pure_i64_batch_inputs = flat_inputs;
        let sum = batch_result?;
        Ok(Some(sum))
    }

    fn collect_i64_map_batch_inputs_from_borrowed_source(
        &self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        arity: usize,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<Option<usize>, RuntimeEvalError> {
        let RuntimeExpr::PureCall { args, .. } = body else {
            unreachable!("i64 map batch shape checked before borrowed source collection");
        };
        let items = match source {
            RuntimeExpr::Value(RuntimeValue::Seq(seq)) => match seq {
                RuntimeSeq::Values(items) => items.as_slice(),
                RuntimeSeq::Dense(_) => {
                    return self.collect_i64_map_batch_inputs_from_i64_source(
                        seq,
                        param,
                        args,
                        arity,
                        flat_inputs,
                    );
                }
                RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => return Ok(None),
            },
            RuntimeExpr::Value(RuntimeValue::Tuple(items)) => items.as_slice(),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Seq(seq)) => match seq {
                    RuntimeSeq::Values(items) => items.as_slice(),
                    RuntimeSeq::Dense(_) => {
                        return self.collect_i64_map_batch_inputs_from_i64_source(
                            seq,
                            param,
                            args,
                            arity,
                            flat_inputs,
                        );
                    }
                    RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => {
                        return Ok(None);
                    }
                },
                Some(RuntimeValue::Tuple(items)) => items.as_slice(),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        if self.collect_i64_map_batch_inputs_without_scope(
            items,
            param,
            args,
            arity,
            flat_inputs,
        )? {
            Ok(Some(items.len()))
        } else {
            Ok(None)
        }
    }

    fn collect_i32_map_batch_inputs_from_borrowed_source(
        &self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        arity: usize,
        flat_inputs: &mut Vec<i32>,
    ) -> Result<Option<usize>, RuntimeEvalError> {
        let RuntimeExpr::PureCall { args, .. } = body else {
            unreachable!("i32 map batch shape checked before borrowed source collection");
        };
        let seq = match source {
            RuntimeExpr::Value(RuntimeValue::Seq(seq)) => seq,
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Seq(seq)) => seq,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let Some(items) = seq.as_i32_slice() else {
            return Ok(None);
        };
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items.iter().copied() {
            for arg in args.iter().take(arity) {
                let Some(value) = self.evaluate_i32_map_arg_from_i32_source(arg, param, item)?
                else {
                    flat_inputs.clear();
                    return Ok(None);
                };
                flat_inputs.push(value);
            }
        }
        Ok(Some(items.len()))
    }

    fn collect_f32_map_batch_inputs_from_borrowed_source(
        &self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        arity: usize,
        flat_inputs: &mut Vec<f32>,
    ) -> Result<Option<usize>, RuntimeEvalError> {
        let RuntimeExpr::PureCall { args, .. } = body else {
            unreachable!("f32 map batch shape checked before borrowed source collection");
        };
        let seq = match source {
            RuntimeExpr::Value(RuntimeValue::Seq(seq)) => seq,
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Seq(seq)) => seq,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let Some(items) = seq.as_f32_slice() else {
            return Ok(None);
        };
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items.iter().copied() {
            for arg in args.iter().take(arity) {
                let Some(value) = self.evaluate_f32_map_arg_from_f32_source(arg, param, item)?
                else {
                    flat_inputs.clear();
                    return Ok(None);
                };
                flat_inputs.push(value);
            }
        }
        Ok(Some(items.len()))
    }

    fn collect_f64_map_batch_inputs_from_borrowed_source(
        &self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        arity: usize,
        flat_inputs: &mut Vec<f64>,
    ) -> Result<Option<usize>, RuntimeEvalError> {
        let RuntimeExpr::PureCall { args, .. } = body else {
            unreachable!("f64 map batch shape checked before borrowed source collection");
        };
        let seq = match source {
            RuntimeExpr::Value(RuntimeValue::Seq(seq)) => seq,
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Seq(seq)) => seq,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let Some(items) = seq.as_f64_slice() else {
            return Ok(None);
        };
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items.iter().copied() {
            for arg in args.iter().take(arity) {
                let Some(value) = self.evaluate_f64_map_arg_from_f64_source(arg, param, item)?
                else {
                    flat_inputs.clear();
                    return Ok(None);
                };
                flat_inputs.push(value);
            }
        }
        Ok(Some(items.len()))
    }

    fn collect_exact_int_map_batch_inputs_from_borrowed_source<T: RuntimePureScalarInteger>(
        &self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        arity: usize,
        flat_inputs: &mut Vec<T>,
    ) -> Result<Option<usize>, RuntimeEvalError> {
        let RuntimeExpr::PureCall { args, .. } = body else {
            unreachable!("exact integer map batch shape checked before borrowed source collection");
        };
        let seq = match source {
            RuntimeExpr::Value(RuntimeValue::Seq(seq)) => seq,
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Seq(seq)) => seq,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let Some(items) = T::seq_slice(seq) else {
            return Ok(None);
        };
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items.iter().copied() {
            for arg in args.iter().take(arity) {
                let Some(value) =
                    self.evaluate_exact_int_map_arg_from_exact_source(arg, param, item)?
                else {
                    flat_inputs.clear();
                    return Ok(None);
                };
                flat_inputs.push(value);
            }
        }
        Ok(Some(items.len()))
    }

    fn i64_map_borrowed_source_len(&self, source: &RuntimeExpr) -> Option<usize> {
        match source {
            RuntimeExpr::Value(RuntimeValue::Seq(seq)) => Some(seq.len()),
            RuntimeExpr::Value(RuntimeValue::Tuple(items)) => Some(items.len()),
            RuntimeExpr::RepeatSeq { len, .. } => Some(*len),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Seq(seq)) => Some(seq.len()),
                Some(RuntimeValue::Tuple(items)) => Some(items.len()),
                _ => None,
            },
            _ => None,
        }
    }

    fn collect_i64_repeated_map_batch_row_from_borrowed_source(
        &self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        arity: usize,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<bool, RuntimeEvalError> {
        let RuntimeExpr::PureCall { args, .. } = body else {
            unreachable!("i64 map batch shape checked before repeated row collection");
        };
        if let RuntimeExpr::RepeatSeq { value, len } = source {
            return self.collect_i64_repeated_map_batch_row_from_repeat_expr(
                value,
                *len,
                param,
                args,
                arity,
                flat_inputs,
            );
        }
        let items = match source {
            RuntimeExpr::Value(RuntimeValue::Seq(seq)) => match seq {
                RuntimeSeq::Values(items) => items.as_slice(),
                RuntimeSeq::Dense(_) => {
                    return self.collect_i64_repeated_map_batch_row_from_i64_source(
                        seq,
                        param,
                        args,
                        arity,
                        flat_inputs,
                    );
                }
                RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => return Ok(false),
            },
            RuntimeExpr::Value(RuntimeValue::Tuple(items)) => items.as_slice(),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Seq(seq)) => match seq {
                    RuntimeSeq::Values(items) => items.as_slice(),
                    RuntimeSeq::Dense(_) => {
                        return self.collect_i64_repeated_map_batch_row_from_i64_source(
                            seq,
                            param,
                            args,
                            arity,
                            flat_inputs,
                        );
                    }
                    RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => {
                        return Ok(false);
                    }
                },
                Some(RuntimeValue::Tuple(items)) => items.as_slice(),
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        self.collect_i64_repeated_map_batch_row_without_scope(
            items,
            param,
            args,
            arity,
            flat_inputs,
        )
    }

    fn collect_i64_repeated_map_batch_row_from_repeat_expr(
        &self,
        value: &RuntimeExpr,
        len: usize,
        param: &str,
        args: &[RuntimeExpr],
        arity: usize,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<bool, RuntimeEvalError> {
        flat_inputs.clear();
        if len == 0 {
            return Ok(true);
        }
        let RuntimeExpr::Value(item) = value else {
            return Ok(false);
        };
        flat_inputs.reserve(arity);
        for arg in args.iter().take(arity) {
            let Some(value) = self.evaluate_i64_map_arg_without_scope(arg, param, item)? else {
                flat_inputs.clear();
                return Ok(false);
            };
            flat_inputs.push(value);
        }
        Ok(true)
    }

    fn evaluate_i64_bracket_seq_sum(
        &mut self,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        match source {
            RuntimeExpr::BracketSeq(items) => {
                let Some((helper_id, arity)) = self.bracket_seq_i64_batch_shape(items) else {
                    return Ok(None);
                };
                let mut flat_inputs = std::mem::take(&mut self.pure_i64_batch_inputs);
                let collect_result = self.collect_i64_pure_batch_inputs(
                    items,
                    arity,
                    pure_backend,
                    &mut flat_inputs,
                );
                if let Err(error) = collect_result {
                    self.pure_i64_batch_inputs = flat_inputs;
                    return Err(error);
                }
                let batch_result = self.call_i64_flat_batch_sum(
                    helper_id,
                    &flat_inputs,
                    arity,
                    items.len(),
                    pure_backend,
                );
                self.pure_i64_batch_inputs = flat_inputs;
                batch_result.map(Some)
            }
            RuntimeExpr::RepeatSeq { value, len } => {
                let RuntimeExpr::PureCall { helper, args } = value.as_ref() else {
                    return Ok(None);
                };
                if helper.0 >= self.plan.pure_helpers.len()
                    || args.len() > RuntimeI64Args::MAX
                    || args
                        .iter()
                        .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
                    || !self
                        .pure_helper_i64_call_shapes
                        .get(helper.0)
                        .copied()
                        .unwrap_or(false)
                {
                    return Ok(None);
                }
                let mut flat_inputs = std::mem::take(&mut self.pure_i64_batch_inputs);
                flat_inputs.clear();
                flat_inputs.reserve(args.len());
                for arg in args {
                    flat_inputs.push(self.evaluate_i64_arg_with_backend(arg, pure_backend)?);
                }
                let batch_result = self.call_i64_repeated_flat_batch_sum(
                    *helper,
                    &flat_inputs,
                    *len,
                    pure_backend,
                );
                self.pure_i64_batch_inputs = flat_inputs;
                batch_result.map(Some)
            }
            _ => Ok(None),
        }
    }

    fn map_i64_batch_shape(
        &self,
        body: &RuntimeExpr,
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let RuntimeExpr::PureCall { helper, args } = body else {
            return None;
        };
        if helper.0 >= self.plan.pure_helpers.len()
            || args.len() > RuntimeI64Args::MAX
            || args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
            || !self
                .pure_helper_i64_call_shapes
                .get(helper.0)
                .copied()
                .unwrap_or(false)
        {
            return None;
        }
        Some((*helper, args.len()))
    }

    fn map_i32_batch_shape(
        &self,
        body: &RuntimeExpr,
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let RuntimeExpr::PureCall { helper, args } = body else {
            return None;
        };
        if helper.0 >= self.plan.pure_helpers.len()
            || args.len() > RuntimeI32Args::MAX
            || args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
            || !self
                .pure_helper_i32_call_shapes
                .get(helper.0)
                .copied()
                .unwrap_or(false)
        {
            return None;
        }
        Some((*helper, args.len()))
    }

    fn map_u32_batch_shape(
        &self,
        body: &RuntimeExpr,
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let RuntimeExpr::PureCall { helper, args } = body else {
            return None;
        };
        if helper.0 >= self.plan.pure_helpers.len()
            || args.len() > RuntimeFixedArgs::<u32>::MAX
            || args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
            || !self
                .pure_helper_u32_call_shapes
                .get(helper.0)
                .copied()
                .unwrap_or(false)
        {
            return None;
        }
        Some((*helper, args.len()))
    }

    fn map_f32_batch_shape(
        &self,
        body: &RuntimeExpr,
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let RuntimeExpr::PureCall { helper, args } = body else {
            return None;
        };
        if helper.0 >= self.plan.pure_helpers.len()
            || args.len() > RuntimeFloat32Args::MAX
            || args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
            || !self
                .pure_helper_f32_call_shapes
                .get(helper.0)
                .copied()
                .unwrap_or(false)
        {
            return None;
        }
        Some((*helper, args.len()))
    }

    fn map_f64_batch_shape(
        &self,
        body: &RuntimeExpr,
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let RuntimeExpr::PureCall { helper, args } = body else {
            return None;
        };
        if helper.0 >= self.plan.pure_helpers.len()
            || args.len() > RuntimeFloat64Args::MAX
            || args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
            || !self
                .pure_helper_f64_call_shapes
                .get(helper.0)
                .copied()
                .unwrap_or(false)
        {
            return None;
        }
        Some((*helper, args.len()))
    }

    fn map_exact_int_batch_shape<T: RuntimePureScalarInteger>(
        &self,
        body: &RuntimeExpr,
    ) -> Option<(crate::plan::RuntimePureHelperId, usize)> {
        let RuntimeExpr::PureCall { helper, args } = body else {
            return None;
        };
        if helper.0 >= self.plan.pure_helpers.len()
            || args.len() > RuntimeFixedArgs::<T>::MAX
            || args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
            || !pure_helper_has_exact_int_call_shape::<T>(&self.plan.pure_helpers[helper.0])
        {
            return None;
        }
        Some((*helper, args.len()))
    }

    fn collect_i64_map_batch_inputs(
        &mut self,
        items: &[RuntimeValue],
        param: &str,
        body: &RuntimeExpr,
        arity: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<(), RuntimeEvalError> {
        let RuntimeExpr::PureCall { args, .. } = body else {
            unreachable!("i64 map batch shape checked before row collection");
        };
        if self.collect_i64_map_batch_inputs_without_scope(
            items,
            param,
            args,
            arity,
            flat_inputs,
        )? {
            return Ok(());
        }
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items {
            self.fiber.env.push_scope_with_capacity(1);
            self.fiber.env.set_ref(param, item);
            for arg in args.iter().take(arity) {
                match self.evaluate_i64_arg_with_backend(arg, pure_backend) {
                    Ok(value) => flat_inputs.push(value),
                    Err(error) => {
                        self.fiber.env.pop_scope();
                        return Err(error);
                    }
                }
            }
            self.fiber.env.pop_scope();
        }
        Ok(())
    }

    fn collect_i64_map_batch_inputs_without_scope(
        &self,
        items: &[RuntimeValue],
        param: &str,
        args: &[RuntimeExpr],
        arity: usize,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<bool, RuntimeEvalError> {
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items {
            for arg in args.iter().take(arity) {
                let Some(value) = self.evaluate_i64_map_arg_without_scope(arg, param, item)? else {
                    flat_inputs.clear();
                    return Ok(false);
                };
                flat_inputs.push(value);
            }
        }
        Ok(true)
    }

    fn collect_i64_map_batch_inputs_from_i64_source(
        &self,
        seq: &RuntimeSeq,
        param: &str,
        args: &[RuntimeExpr],
        arity: usize,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<Option<usize>, RuntimeEvalError> {
        flat_inputs.clear();
        flat_inputs.reserve(seq.len().saturating_mul(arity));
        let mut accepted = true;
        let compatible = seq.try_for_each_i64(|item| {
            if !accepted {
                return Ok(());
            }
            for arg in args.iter().take(arity) {
                let Some(value) = self.evaluate_i64_map_arg_from_i64_source(arg, param, item)?
                else {
                    flat_inputs.clear();
                    accepted = false;
                    return Ok(());
                };
                flat_inputs.push(value);
            }
            Ok(())
        })?;
        if compatible && accepted {
            Ok(Some(seq.len()))
        } else {
            Ok(None)
        }
    }

    fn collect_i64_repeated_map_batch_row_without_scope(
        &self,
        items: &[RuntimeValue],
        param: &str,
        args: &[RuntimeExpr],
        arity: usize,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<bool, RuntimeEvalError> {
        flat_inputs.clear();
        let Some(first) = items.first() else {
            return Ok(true);
        };
        flat_inputs.reserve(arity);
        for arg in args.iter().take(arity) {
            let Some(value) = self.evaluate_i64_map_arg_without_scope(arg, param, first)? else {
                flat_inputs.clear();
                return Ok(false);
            };
            flat_inputs.push(value);
        }
        for item in &items[1..] {
            for (index, arg) in args.iter().take(arity).enumerate() {
                let Some(value) = self.evaluate_i64_map_arg_without_scope(arg, param, item)? else {
                    flat_inputs.clear();
                    return Ok(false);
                };
                if flat_inputs.get(index).copied() != Some(value) {
                    flat_inputs.clear();
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn collect_i64_repeated_map_batch_row_from_i64_source(
        &self,
        seq: &RuntimeSeq,
        param: &str,
        args: &[RuntimeExpr],
        arity: usize,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<bool, RuntimeEvalError> {
        flat_inputs.clear();
        let Some(first) = seq.first_i64() else {
            return Ok(false);
        };
        let Some(first) = first else {
            return Ok(true);
        };
        flat_inputs.reserve(arity);
        for arg in args.iter().take(arity) {
            let Some(value) = self.evaluate_i64_map_arg_from_i64_source(arg, param, first)? else {
                flat_inputs.clear();
                return Ok(false);
            };
            flat_inputs.push(value);
        }
        let mut index = 0_usize;
        let mut same_row = true;
        let compatible = seq.try_for_each_i64(|item| {
            if !same_row {
                return Ok(());
            }
            if index == 0 {
                index += 1;
                return Ok(());
            }
            for (index, arg) in args.iter().take(arity).enumerate() {
                let Some(value) = self.evaluate_i64_map_arg_from_i64_source(arg, param, item)?
                else {
                    flat_inputs.clear();
                    same_row = false;
                    return Ok(());
                };
                if flat_inputs.get(index).copied() != Some(value) {
                    flat_inputs.clear();
                    same_row = false;
                    return Ok(());
                }
            }
            index += 1;
            Ok(())
        })?;
        Ok(compatible && same_row)
    }

    fn evaluate_i64_map_arg_without_scope(
        &self,
        expr: &RuntimeExpr,
        param: &str,
        item: &RuntimeValue,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(value.exact_i64()),
            RuntimeExpr::Local(name) if name == param => match item {
                RuntimeValue::Int(value) => Ok(value.exact_i64()),
                value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
            },
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Int(value)) => Ok(value.exact_i64()),
                Some(value) => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => Ok(None),
        }
    }

    fn evaluate_i64_map_arg_from_i64_source(
        &self,
        expr: &RuntimeExpr,
        param: &str,
        item: i64,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(value.exact_i64()),
            RuntimeExpr::Local(name) if name == param => Ok(Some(item)),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Int(value)) => Ok(value.exact_i64()),
                Some(value) => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => Ok(None),
        }
    }

    fn evaluate_i32_map_arg_from_i32_source(
        &self,
        expr: &RuntimeExpr,
        param: &str,
        item: i32,
    ) -> Result<Option<i32>, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::Int(value)) => {
                value
                    .exact_i32()
                    .map(Some)
                    .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                        name: "i32 map batch".to_owned(),
                        reason: format!("literal `{value}` is outside i32 range"),
                    })
            }
            RuntimeExpr::Local(name) if name == param => Ok(Some(item)),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Int(value)) => {
                    value
                        .exact_i32()
                        .map(Some)
                        .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                            name: "i32 map batch".to_owned(),
                            reason: format!(
                                "binding `{name}` value `{value}` is outside i32 range"
                            ),
                        })
                }
                Some(value) => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => Ok(None),
        }
    }

    fn evaluate_f32_map_arg_from_f32_source(
        &self,
        expr: &RuntimeExpr,
        param: &str,
        item: f32,
    ) -> Result<Option<f32>, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::F32(value)) => Ok(Some(*value)),
            RuntimeExpr::Local(name) if name == param => Ok(Some(item)),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::F32(value)) => Ok(Some(*value)),
                Some(value) => Err(RuntimeEvalError::UnsupportedPure {
                    name: "f32 map batch".to_owned(),
                    reason: format!("expected f32 argument, got {}", runtime_value_label(value)),
                }),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => Ok(None),
        }
    }

    fn evaluate_f64_map_arg_from_f64_source(
        &self,
        expr: &RuntimeExpr,
        param: &str,
        item: f64,
    ) -> Result<Option<f64>, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::F64(value)) => Ok(Some(*value)),
            RuntimeExpr::Local(name) if name == param => Ok(Some(item)),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::F64(value)) => Ok(Some(*value)),
                Some(value) => Err(RuntimeEvalError::UnsupportedPure {
                    name: "f64 map batch".to_owned(),
                    reason: format!("expected f64 argument, got {}", runtime_value_label(value)),
                }),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => Ok(None),
        }
    }

    fn evaluate_exact_int_map_arg_from_exact_source<T: RuntimePureScalarInteger>(
        &self,
        expr: &RuntimeExpr,
        param: &str,
        item: T,
    ) -> Result<Option<T>, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(value) => {
                T::try_from_runtime_value("exact integer map batch", value.clone()).map(Some)
            }
            RuntimeExpr::Local(name) if name == param => Ok(Some(item)),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(value) => {
                    T::try_from_runtime_value("exact integer map batch", value.clone()).map(Some)
                }
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => Ok(None),
        }
    }

    pub(super) fn evaluate_call_args(
        &mut self,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            match arg {
                RuntimeExpr::SpreadArg(expr) => {
                    let spread = self.evaluate_expr_with_backend(expr, pure_backend)?;
                    values.extend(spread_runtime_values(spread)?);
                }
                expr => values.push(self.evaluate_expr_with_backend(expr, pure_backend)?),
            }
        }
        Ok(values)
    }
}
