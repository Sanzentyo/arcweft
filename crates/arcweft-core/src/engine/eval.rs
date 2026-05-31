use super::{
    Engine, FlowFiberStatus, RuntimeBinding, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr,
    RuntimeExprMatchArm, RuntimeF32, RuntimeF64, RuntimeFieldValue, RuntimeMatchArm,
    RuntimeMatchSelection, RuntimePattern, RuntimeSeq, RuntimeStepOutput, RuntimeValue,
    evaluate_binary, evaluate_unary, match_runtime_pattern, runtime_sequence_dense_i32,
    runtime_sequence_dense_i64, runtime_sequence_from_literal_values,
    runtime_sequence_repeat_value, runtime_sequence_values, runtime_value_into_sequence_values,
    runtime_value_label, sum_i64_sequence_ref,
};
use crate::plan::{RuntimePureInputType, RuntimePureOutputType};
use crate::pure::{
    RuntimeF32Args, RuntimeF64Args, RuntimeFixedArgs, RuntimeI32Args, RuntimeI64Args,
    RuntimePureCallBackend, RuntimePureScalarInteger, VmRuntimePureCallBackend,
};
use crate::value::RuntimeBinaryOp;
use crate::value::RuntimeFieldExpr;

impl Engine {
    pub(super) fn evaluate_let_with_backend(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) {
        match self
            .evaluate_expr_with_backend(expr, pure_backend)
            .and_then(|value| {
                self.try_bind_pattern(pattern, &value)
                    .map(|matched| (matched, value))
            }) {
            Ok((true, _)) => {}
            Ok((false, value)) => {
                self.fail_eval(
                    RuntimeEvalError::PatternMismatch(runtime_value_label(&value)),
                    output,
                );
            }
            Err(error) => self.fail_eval(error, output),
        }
    }

    pub(super) fn evaluate_if_let_with_backend(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<Vec<RuntimeBinding>>, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        let Some(bindings) = match_runtime_pattern(pattern, &value)? else {
            return Ok(None);
        };
        if let Some(guard) = guard {
            let matched = self.with_temp_bindings_ref(&bindings, |this| {
                this.evaluate_bool_with_backend(guard, pure_backend)
            })?;
            if !matched {
                return Ok(None);
            }
        }
        Ok(Some(bindings))
    }

    pub(super) fn evaluate_match_with_backend(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: Vec<RuntimeMatchArm>,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeMatchSelection, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(scrutinee, pure_backend)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            if let Some(guard) = arm.guard.as_ref()
                && !self.with_temp_bindings_ref(&bindings, |this| {
                    this.evaluate_bool_with_backend(guard, pure_backend)
                })?
            {
                continue;
            }
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
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(value) => Ok(value.clone()),
            RuntimeExpr::Local(name) => self
                .fiber
                .env
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeEvalError::UnknownBinding(name.clone())),
            RuntimeExpr::EntityRef(target) => Ok(RuntimeValue::EntityRef(target.clone())),
            RuntimeExpr::Let { name, expr, body } => {
                self.evaluate_let_expr(name, expr, body, pure_backend)
            }
            RuntimeExpr::Tuple(_)
            | RuntimeExpr::BracketSeq(_)
            | RuntimeExpr::RepeatSeq { .. }
            | RuntimeExpr::Record(_)
            | RuntimeExpr::Variant { .. }
            | RuntimeExpr::Field { .. } => self.evaluate_data_expr(expr, pure_backend),
            RuntimeExpr::SpreadArg(_) => Err(RuntimeEvalError::SpreadOutsideCall),
            RuntimeExpr::Call { callee, args } => {
                self.evaluate_call_expr(callee, args, pure_backend)
            }
            RuntimeExpr::PureCall { helper, args } => {
                self.evaluate_pure_call_expr(*helper, args, pure_backend)
            }
            RuntimeExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.evaluate_method_call_expr(receiver, method, args, pure_backend),
            RuntimeExpr::Map {
                source,
                param,
                body,
            } => self.evaluate_map_expr(source, param, body, pure_backend),
            RuntimeExpr::Sum { source } => self.evaluate_sum_expr(source, pure_backend),
            RuntimeExpr::Unary { op, expr } => {
                let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
                evaluate_unary(*op, value)
            }
            RuntimeExpr::Binary { lhs, op, rhs } => {
                let lhs = self.evaluate_expr_with_backend(lhs, pure_backend)?;
                let rhs = self.evaluate_expr_with_backend(rhs, pure_backend)?;
                evaluate_binary(lhs, *op, rhs)
            }
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.evaluate_bool_with_backend(condition, pure_backend)? {
                    self.evaluate_expr_with_backend(then_expr, pure_backend)
                } else {
                    self.evaluate_expr_with_backend(else_expr, pure_backend)
                }
            }
            RuntimeExpr::IfLet {
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
            RuntimeExpr::Match { scrutinee, arms } => {
                self.evaluate_match_expr(scrutinee, arms, pure_backend)
            }
        }
    }

    fn evaluate_data_expr(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Tuple(items) => items
                .iter()
                .map(|item| self.evaluate_expr_with_backend(item, pure_backend))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Tuple),
            RuntimeExpr::BracketSeq(items) => self.evaluate_bracket_seq_expr(items, pure_backend),
            RuntimeExpr::RepeatSeq { value, len } => {
                self.evaluate_repeat_seq_expr(value, *len, pure_backend)
            }
            RuntimeExpr::Record(fields) => self.evaluate_record_expr(fields, pure_backend),
            RuntimeExpr::Variant {
                path,
                name,
                payload,
            } => Ok(RuntimeValue::Variant {
                path: path.clone(),
                name: name.clone(),
                payload: payload
                    .as_ref()
                    .map(|expr| {
                        self.evaluate_expr_with_backend(expr, pure_backend)
                            .map(Box::new)
                    })
                    .transpose()?,
            }),
            RuntimeExpr::Field { target, field } => {
                self.evaluate_field_expr(target, field, pure_backend)
            }
            _ => unreachable!("data expression helper received non-data expression"),
        }
    }

    fn evaluate_bracket_seq_expr(
        &mut self,
        items: &[RuntimeExpr],
        pure_backend: &mut impl RuntimePureCallBackend,
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

    fn evaluate_repeat_seq_expr(
        &mut self,
        value: &RuntimeExpr,
        len: usize,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let RuntimeExpr::Value(value) = value {
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
        let (first_helper, first_args) = match items.first()? {
            RuntimeExpr::PureCall { helper, args } => (*helper, args),
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
            .all(|item| match item {
                RuntimeExpr::PureCall { helper, args } => {
                    *helper == first_helper
                        && args.len() == arity
                        && args
                            .iter()
                            .all(|arg| !matches!(arg, RuntimeExpr::SpreadArg(_)))
                }
                _ => false,
            })
            .then_some((first_helper, arity))
    }

    fn collect_i64_pure_batch_inputs(
        &mut self,
        items: &[RuntimeExpr],
        arity: usize,
        pure_backend: &mut impl RuntimePureCallBackend,
        flat_inputs: &mut Vec<i64>,
    ) -> Result<(), RuntimeEvalError> {
        flat_inputs.clear();
        flat_inputs.reserve(items.len().saturating_mul(arity));
        for item in items {
            let RuntimeExpr::PureCall { args, .. } = item else {
                unreachable!("i64 pure batch shape checked before row collection");
            };
            for arg in args.iter().take(arity) {
                flat_inputs.push(self.evaluate_i64_arg_with_backend(arg, pure_backend)?);
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
        pure_backend: &mut impl RuntimePureCallBackend,
        map_outputs: impl FnOnce(&[i64]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_i64_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
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

    fn call_i32_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[i32],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimePureCallBackend,
        map_outputs: impl FnOnce(&[i32]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_i32_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_i32_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_i32_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_i32_batch_outputs = out;
        Ok(result)
    }

    fn call_i64_flat_batch_sum(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[i64],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<i64, RuntimeEvalError> {
        let helper = &self.plan.pure_helpers[helper_id.0];
        pure_backend.call_i64_flat_batch_sum(helper, flat_inputs, arity, row_count)
    }

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        row: &[i64],
        row_count: usize,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<i64, RuntimeEvalError> {
        let helper = &self.plan.pure_helpers[helper_id.0];
        pure_backend.call_i64_repeated_flat_batch_sum(helper, row, row_count)
    }

    fn evaluate_record_expr(
        &mut self,
        fields: &[RuntimeFieldExpr],
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        fields
            .iter()
            .map(|field| {
                Ok(RuntimeFieldValue {
                    name: field.name.clone(),
                    value: self.evaluate_expr_with_backend(&field.value, pure_backend)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeValue::Record)
    }

    fn evaluate_field_expr(
        &mut self,
        target: &RuntimeExpr,
        field: &str,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(target, pure_backend)?;
        match value {
            RuntimeValue::Record(fields) => fields
                .into_iter()
                .find(|candidate| candidate.name == field)
                .map(|field| field.value)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.to_owned(),
                    value: "record".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: field.to_owned(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_call_expr(
        &mut self,
        callee: &str,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let args = self.evaluate_call_args(args, pure_backend)?;
        Ok(evaluate_runtime_call(callee, &args))
    }

    fn evaluate_pure_call_expr(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimePureCallBackend,
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
                return Ok(RuntimeValue::Int(value));
            }
        }
        if self
            .pure_helper_f32_call_shapes
            .get(helper_id.0)
            .copied()
            .unwrap_or(false)
            && args.len() <= RuntimeF32Args::MAX
            && !args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
        {
            let mut values = [RuntimeF32::from_bits(0); RuntimeF32Args::MAX];
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
            && args.len() <= RuntimeF64Args::MAX
            && !args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
        {
            let mut values = [RuntimeF64::from_bits(0); RuntimeF64Args::MAX];
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

    fn evaluate_i64_arg_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<i64, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(*value),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Int(value)) => Ok(*value),
                Some(value) => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            _ => match self.evaluate_expr_with_backend(expr, pure_backend)? {
                RuntimeValue::Int(value) => Ok(value),
                value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
            },
        }
    }

    fn evaluate_f32_arg_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeF32, RuntimeEvalError> {
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
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeF64, RuntimeEvalError> {
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

    fn evaluate_method_call_expr(
        &mut self,
        receiver: &RuntimeExpr,
        method: &str,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let receiver = self.evaluate_expr_with_backend(receiver, pure_backend)?;
        let args = self.evaluate_call_args(args, pure_backend)?;
        Ok(evaluate_runtime_method_call(receiver, method, &args))
    }

    fn evaluate_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Some(value) = self.evaluate_i32_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        if let Some(value) = self.evaluate_i64_map_expr(source, param, body, pure_backend)? {
            return Ok(value);
        }
        let items = match runtime_value_into_sequence_values(
            self.evaluate_expr_with_backend(source, pure_backend)?,
        ) {
            Ok(items) => items,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        items
            .iter()
            .map(|item| {
                self.with_temp_binding_ref(param, item, |this| {
                    this.evaluate_expr_with_backend(body, pure_backend)
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_values)
    }

    fn evaluate_i32_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
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

    fn evaluate_i64_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
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

    fn evaluate_sum_expr(
        &mut self,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Some(sum) = self.evaluate_map_sum_expr(source, pure_backend)? {
            return Ok(RuntimeValue::Int(sum));
        }
        if let Some(sum) = self.evaluate_i64_bracket_seq_sum(source, pure_backend)? {
            return Ok(RuntimeValue::Int(sum));
        }
        if let RuntimeExpr::Local(name) = source
            && let Some(sum) = self.evaluate_i64_local_sequence_sum(name)?
        {
            return Ok(RuntimeValue::Int(sum));
        }
        let value = self.evaluate_expr_with_backend(source, pure_backend)?;
        if let RuntimeValue::Seq(seq) = &value
            && let Some(sum) = seq.sum_as_i64()
        {
            return Ok(RuntimeValue::Int(sum));
        }
        let items = match runtime_value_into_sequence_values(value) {
            Ok(items) => items,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        items
            .into_iter()
            .try_fold(RuntimeValue::Int(0), |acc, item| {
                evaluate_binary(acc, RuntimeBinaryOp::Add, item)
            })
    }

    fn evaluate_map_sum_expr(
        &mut self,
        source: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
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
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_i8_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<i8>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_i8_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_i16_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_i16_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<i16>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_i16_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_i32_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
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
                let batch_result = pure_backend.call_exact_int_flat_batch_sum::<i32>(
                    helper,
                    &flat_inputs,
                    arity,
                    row_count,
                );
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
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_i128_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<i128>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_i128_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u8_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_u8_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<u8>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_u8_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u16_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_u16_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<u16>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_u16_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u32_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_u32_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<u32>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_u32_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u64_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_u64_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<u64>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_u64_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_u128_map_sum(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let mut flat_inputs = std::mem::take(&mut self.pure_u128_batch_inputs);
        let result = self.evaluate_exact_int_map_sum_with_inputs::<u128>(
            source,
            param,
            body,
            pure_backend,
            &mut flat_inputs,
        );
        self.pure_u128_batch_inputs = flat_inputs;
        result
    }

    fn evaluate_exact_int_map_sum_with_inputs<T: RuntimePureScalarInteger>(
        &self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
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
        pure_backend: &mut impl RuntimePureCallBackend,
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
        pure_backend: &mut impl RuntimePureCallBackend,
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
        pure_backend: &mut impl RuntimePureCallBackend,
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
            RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(Some(*value)),
            RuntimeExpr::Local(name) if name == param => match item {
                RuntimeValue::Int(value) => Ok(Some(*value)),
                value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
            },
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Int(value)) => Ok(Some(*value)),
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
            RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(Some(*value)),
            RuntimeExpr::Local(name) if name == param => Ok(Some(item)),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Int(value)) => Ok(Some(*value)),
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
            RuntimeExpr::Value(RuntimeValue::Int(value)) => i32::try_from(*value)
                .map(Some)
                .map_err(|_| RuntimeEvalError::UnsupportedPure {
                    name: "i32 map batch".to_owned(),
                    reason: format!("literal `{value}` is outside i32 range"),
                }),
            RuntimeExpr::Local(name) if name == param => Ok(Some(item)),
            RuntimeExpr::Local(name) => match self.fiber.env.get(name) {
                Some(RuntimeValue::Int(value)) => {
                    i32::try_from(*value)
                        .map(Some)
                        .map_err(|_| RuntimeEvalError::UnsupportedPure {
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

    fn evaluate_call_args(
        &mut self,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimePureCallBackend,
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

    fn evaluate_let_expr(
        &mut self,
        name: &str,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        self.fiber.env.push_scope_with_capacity(1);
        self.fiber.env.set(name.to_owned(), value);
        let result = self.evaluate_expr_with_backend(body, pure_backend);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn evaluate_if_let_expr(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        let Some(bindings) = match_runtime_pattern(pattern, &value)? else {
            return self.evaluate_expr_with_backend(else_expr, pure_backend);
        };
        let guard_matched = if let Some(guard) = guard {
            self.with_temp_bindings_ref(&bindings, |this| {
                this.evaluate_bool_with_backend(guard, pure_backend)
            })?
        } else {
            true
        };
        if guard_matched {
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
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(scrutinee, pure_backend)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            if let Some(guard) = arm.guard.as_ref()
                && !self.with_temp_bindings_ref(&bindings, |this| {
                    this.evaluate_bool_with_backend(guard, pure_backend)
                })?
            {
                continue;
            }
            return self.with_temp_bindings(bindings, |this| {
                this.evaluate_expr_with_backend(&arm.value, pure_backend)
            });
        }
        Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
            &value,
        )))
    }

    pub(super) fn evaluate_bool_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimePureCallBackend,
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
        I: IntoIterator<Item = RuntimeBinding>,
        I::IntoIter: ExactSizeIterator,
    {
        let bindings = bindings.into_iter();
        self.fiber.env.push_scope_with_capacity(bindings.len());
        self.fiber.env.bind_all(bindings);
        let result = f(self);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn with_temp_bindings_ref<T>(
        &mut self,
        bindings: &[RuntimeBinding],
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.fiber.env.push_scope_with_capacity(bindings.len());
        self.fiber.env.bind_all_ref(bindings);
        let result = f(self);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn with_temp_binding_ref<T>(
        &mut self,
        name: &str,
        value: &RuntimeValue,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.fiber.env.push_scope_with_capacity(1);
        self.fiber.env.set_ref(name, value);
        let result = f(self);
        self.fiber.env.pop_scope();
        result
    }

    pub(super) fn evaluate_entity_target(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<String, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::EntityRef(target) | RuntimeValue::String(target) => Ok(target),
            value => Err(RuntimeEvalError::ExpectedEntityRef(runtime_value_label(
                &value,
            ))),
        }
    }

    pub(super) fn try_bind_pattern(
        &mut self,
        pattern: &RuntimePattern,
        value: &RuntimeValue,
    ) -> Result<bool, RuntimeEvalError> {
        let Some(bindings) = match_runtime_pattern(pattern, value)? else {
            return Ok(false);
        };
        self.fiber.env.bind_all(bindings);
        Ok(true)
    }

    pub(super) fn fail_eval(
        &mut self,
        error: impl std::fmt::Display,
        output: &mut RuntimeStepOutput,
    ) {
        let message = error.to_string();
        self.fiber.status = FlowFiberStatus::Failed(message.clone());
        output.diagnostics.push(RuntimeDiagnostic { message });
    }
}

pub(super) fn pure_helper_has_i64_call_shape(helper: &crate::plan::RuntimePureHelper) -> bool {
    if helper.output_type != RuntimePureOutputType::I64 || !pure_helper_has_only_i64_inputs(helper)
    {
        return false;
    }
    let mut int_names = helper
        .input_names
        .iter()
        .zip(helper.input_types.iter())
        .filter_map(|(name, ty)| {
            matches!(ty, crate::plan::RuntimePureInputType::I64).then_some(name.as_str())
        })
        .collect::<Vec<_>>();
    expr_returns_i64(&helper.expr, &mut int_names)
}

pub(super) fn pure_helper_has_i32_call_shape(helper: &crate::plan::RuntimePureHelper) -> bool {
    if helper.output_type != RuntimePureOutputType::I32 || !pure_helper_has_only_i32_inputs(helper)
    {
        return false;
    }
    let mut int_names = helper
        .input_names
        .iter()
        .zip(helper.input_types.iter())
        .filter_map(|(name, ty)| matches!(ty, RuntimePureInputType::I32).then_some(name.as_str()))
        .collect::<Vec<_>>();
    expr_returns_i64(&helper.expr, &mut int_names)
}

pub(super) fn pure_helper_has_f32_call_shape(helper: &crate::plan::RuntimePureHelper) -> bool {
    if helper.output_type != RuntimePureOutputType::F32 || !pure_helper_has_only_f32_inputs(helper)
    {
        return false;
    }
    let mut float_names = helper
        .input_names
        .iter()
        .zip(helper.input_types.iter())
        .filter_map(|(name, ty)| matches!(ty, RuntimePureInputType::F32).then_some(name.as_str()))
        .collect::<Vec<_>>();
    expr_returns_f32(&helper.expr, &mut float_names)
}

pub(super) fn pure_helper_has_f64_call_shape(helper: &crate::plan::RuntimePureHelper) -> bool {
    if helper.output_type != RuntimePureOutputType::F64 || !pure_helper_has_only_f64_inputs(helper)
    {
        return false;
    }
    let mut float_names = helper
        .input_names
        .iter()
        .zip(helper.input_types.iter())
        .filter_map(|(name, ty)| matches!(ty, RuntimePureInputType::F64).then_some(name.as_str()))
        .collect::<Vec<_>>();
    expr_returns_f64(&helper.expr, &mut float_names)
}

fn pure_helper_has_exact_int_call_shape<T: RuntimePureScalarInteger>(
    helper: &crate::plan::RuntimePureHelper,
) -> bool {
    if helper.output_type != T::OUTPUT_TYPE || !pure_helper_has_only_exact_int_inputs::<T>(helper) {
        return false;
    }
    let mut int_names = helper
        .input_names
        .iter()
        .zip(helper.input_types.iter())
        .filter_map(|(name, ty)| (*ty == T::INPUT_TYPE).then_some(name.as_str()))
        .collect::<Vec<_>>();
    expr_returns_i64(&helper.expr, &mut int_names)
}

fn pure_helper_has_only_i64_inputs(helper: &crate::plan::RuntimePureHelper) -> bool {
    helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::I64))
}

fn pure_helper_has_only_i32_inputs(helper: &crate::plan::RuntimePureHelper) -> bool {
    helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::I32))
}

fn pure_helper_has_only_f32_inputs(helper: &crate::plan::RuntimePureHelper) -> bool {
    helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::F32))
}

fn pure_helper_has_only_f64_inputs(helper: &crate::plan::RuntimePureHelper) -> bool {
    helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::F64))
}

fn pure_helper_has_only_exact_int_inputs<T: RuntimePureScalarInteger>(
    helper: &crate::plan::RuntimePureHelper,
) -> bool {
    helper.input_names.len() == helper.input_types.len()
        && helper.input_types.iter().all(|ty| *ty == T::INPUT_TYPE)
}

fn expr_returns_i64<'a>(expr: &'a RuntimeExpr, int_names: &mut Vec<&'a str>) -> bool {
    match expr {
        RuntimeExpr::Value(
            RuntimeValue::Int(_)
            | RuntimeValue::I128(_)
            | RuntimeValue::UInt(_)
            | RuntimeValue::U128(_),
        ) => true,
        RuntimeExpr::Local(name) => int_names.contains(&name.as_str()),
        RuntimeExpr::Let { name, expr, body } => {
            if !expr_returns_i64(expr, int_names) {
                return false;
            }
            let original_len = int_names.len();
            int_names.push(name.as_str());
            let returns_i64 = expr_returns_i64(body, int_names);
            int_names.truncate(original_len);
            returns_i64
        }
        RuntimeExpr::Call { callee, args } if callee == "add" => {
            args.iter().all(|arg| expr_returns_i64(arg, int_names))
        }
        RuntimeExpr::Unary {
            op: crate::value::RuntimeUnaryOp::Neg,
            expr,
        } => expr_returns_i64(expr, int_names),
        RuntimeExpr::Binary {
            lhs,
            op:
                RuntimeBinaryOp::Add
                | RuntimeBinaryOp::Sub
                | RuntimeBinaryOp::Mul
                | RuntimeBinaryOp::Div,
            rhs,
        } => expr_returns_i64(lhs, int_names) && expr_returns_i64(rhs, int_names),
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_returns_bool(condition, int_names)
                && expr_returns_i64(then_expr, int_names)
                && expr_returns_i64(else_expr, int_names)
        }
        _ => false,
    }
}

fn expr_returns_f32<'a>(expr: &'a RuntimeExpr, float_names: &mut Vec<&'a str>) -> bool {
    match expr {
        RuntimeExpr::Value(RuntimeValue::F32(_)) => true,
        RuntimeExpr::Local(name) => float_names.contains(&name.as_str()),
        RuntimeExpr::Let { name, expr, body } => {
            if !expr_returns_f32(expr, float_names) {
                return false;
            }
            let original_len = float_names.len();
            float_names.push(name.as_str());
            let returns_f32 = expr_returns_f32(body, float_names);
            float_names.truncate(original_len);
            returns_f32
        }
        RuntimeExpr::Unary {
            op: crate::value::RuntimeUnaryOp::Neg,
            expr,
        } => expr_returns_f32(expr, float_names),
        RuntimeExpr::Binary {
            lhs,
            op:
                RuntimeBinaryOp::Add
                | RuntimeBinaryOp::Sub
                | RuntimeBinaryOp::Mul
                | RuntimeBinaryOp::Div,
            rhs,
        } => expr_returns_f32(lhs, float_names) && expr_returns_f32(rhs, float_names),
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_returns_float_bool(condition, float_names, expr_returns_f32)
                && expr_returns_f32(then_expr, float_names)
                && expr_returns_f32(else_expr, float_names)
        }
        _ => false,
    }
}

fn expr_returns_f64<'a>(expr: &'a RuntimeExpr, float_names: &mut Vec<&'a str>) -> bool {
    match expr {
        RuntimeExpr::Value(RuntimeValue::F64(_)) => true,
        RuntimeExpr::Local(name) => float_names.contains(&name.as_str()),
        RuntimeExpr::Let { name, expr, body } => {
            if !expr_returns_f64(expr, float_names) {
                return false;
            }
            let original_len = float_names.len();
            float_names.push(name.as_str());
            let returns_f64 = expr_returns_f64(body, float_names);
            float_names.truncate(original_len);
            returns_f64
        }
        RuntimeExpr::Unary {
            op: crate::value::RuntimeUnaryOp::Neg,
            expr,
        } => expr_returns_f64(expr, float_names),
        RuntimeExpr::Binary {
            lhs,
            op:
                RuntimeBinaryOp::Add
                | RuntimeBinaryOp::Sub
                | RuntimeBinaryOp::Mul
                | RuntimeBinaryOp::Div,
            rhs,
        } => expr_returns_f64(lhs, float_names) && expr_returns_f64(rhs, float_names),
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_returns_float_bool(condition, float_names, expr_returns_f64)
                && expr_returns_f64(then_expr, float_names)
                && expr_returns_f64(else_expr, float_names)
        }
        _ => false,
    }
}

fn expr_returns_float_bool<'a>(
    expr: &'a RuntimeExpr,
    float_names: &mut Vec<&'a str>,
    expr_returns_float: fn(&'a RuntimeExpr, &mut Vec<&'a str>) -> bool,
) -> bool {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(_)) => true,
        RuntimeExpr::Unary {
            op: crate::value::RuntimeUnaryOp::Not,
            expr,
        } => expr_returns_float_bool(expr, float_names, expr_returns_float),
        RuntimeExpr::Binary {
            lhs,
            op:
                RuntimeBinaryOp::Eq
                | RuntimeBinaryOp::Ne
                | RuntimeBinaryOp::Lt
                | RuntimeBinaryOp::Le
                | RuntimeBinaryOp::Gt
                | RuntimeBinaryOp::Ge,
            rhs,
        } => expr_returns_float(lhs, float_names) && expr_returns_float(rhs, float_names),
        RuntimeExpr::Binary {
            lhs,
            op: RuntimeBinaryOp::And | RuntimeBinaryOp::Or,
            rhs,
        } => {
            expr_returns_float_bool(lhs, float_names, expr_returns_float)
                && expr_returns_float_bool(rhs, float_names, expr_returns_float)
        }
        _ => false,
    }
}

fn expr_returns_bool<'a>(expr: &'a RuntimeExpr, int_names: &mut Vec<&'a str>) -> bool {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(_)) => true,
        RuntimeExpr::Unary {
            op: crate::value::RuntimeUnaryOp::Not,
            expr,
        } => expr_returns_bool(expr, int_names),
        RuntimeExpr::Binary {
            lhs,
            op:
                RuntimeBinaryOp::Eq
                | RuntimeBinaryOp::Ne
                | RuntimeBinaryOp::Lt
                | RuntimeBinaryOp::Le
                | RuntimeBinaryOp::Gt
                | RuntimeBinaryOp::Ge,
            rhs,
        } => expr_returns_i64(lhs, int_names) && expr_returns_i64(rhs, int_names),
        RuntimeExpr::Binary {
            lhs,
            op: RuntimeBinaryOp::And | RuntimeBinaryOp::Or,
            rhs,
        } => expr_returns_bool(lhs, int_names) && expr_returns_bool(rhs, int_names),
        _ => false,
    }
}

fn spread_runtime_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
    match runtime_value_into_sequence_values(value) {
        Ok(items) => Ok(items),
        Err(value) => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
    }
}

fn evaluate_runtime_call(callee: &str, args: &[RuntimeValue]) -> RuntimeValue {
    match (callee, args) {
        ("add", [RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)]) => {
            RuntimeValue::Int(lhs.saturating_add(*rhs))
        }
        (
            "path.save" | "path.asset" | "path.temp" | "path.export",
            [RuntimeValue::String(path)],
        ) => {
            let space = callee.strip_prefix("path.").unwrap_or(callee);
            RuntimeValue::String(format!("{space}:{path}"))
        }
        _ => RuntimeValue::String(format!(
            "{callee}({})",
            args.iter()
                .map(runtime_value_label)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn evaluate_runtime_method_call(
    receiver: RuntimeValue,
    method: &str,
    args: &[RuntimeValue],
) -> RuntimeValue {
    match (receiver, method, args) {
        (RuntimeValue::String(value), "trim", []) => {
            RuntimeValue::String(value.trim_matches(char::is_whitespace).to_owned())
        }
        (RuntimeValue::String(value), "to_string", []) => RuntimeValue::String(value),
        (RuntimeValue::Seq(seq), "len", []) => runtime_len_value(seq.len()),
        (RuntimeValue::Tuple(items), "len", []) => runtime_len_value(items.len()),
        (receiver, method, args) => RuntimeValue::String(format!(
            "{}.{method}({})",
            runtime_value_label(&receiver),
            args.iter()
                .map(runtime_value_label)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn runtime_len_value(len: usize) -> RuntimeValue {
    RuntimeValue::UInt(u64::try_from(len).unwrap_or(u64::MAX))
}
