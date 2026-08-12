use super::{
    Engine, FlowFiberStatus, RuntimeBinding, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr,
    RuntimeExprMatchArm, RuntimeFieldValue, RuntimeFunctionValue, RuntimeMatchArm,
    RuntimeMatchSelection, RuntimePattern, RuntimeSeq, RuntimeStepOutput, RuntimeValue,
    evaluate_binary, evaluate_unary, match_runtime_pattern, runtime_sequence_dense_f32,
    runtime_sequence_dense_f64, runtime_sequence_dense_i8, runtime_sequence_dense_i16,
    runtime_sequence_dense_i32, runtime_sequence_dense_i64, runtime_sequence_dense_i128,
    runtime_sequence_dense_u8, runtime_sequence_dense_u16, runtime_sequence_dense_u32,
    runtime_sequence_dense_u64, runtime_sequence_dense_u128, runtime_sequence_from_literal_values,
    runtime_sequence_repeat_value, runtime_sequence_values, runtime_value_into_sequence_values,
    runtime_value_label, sum_i64_sequence_ref,
};
use crate::plan::{FlowRuntimeId, RuntimePureInputType, RuntimePureOutputType};
use crate::pure::{
    RuntimeCallBackend, RuntimeFixedArgs, RuntimeFloat32Args, RuntimeFloat64Args, RuntimeI32Args,
    RuntimeI64Args, RuntimePureCallBackend, RuntimePureScalarInteger, VmRuntimePureCallBackend,
};
use crate::value::{RuntimeBinaryOp, RuntimeExactInteger, RuntimeFieldExpr};
use crate::value::{
    RuntimeCallTarget, RuntimeIntrinsic, evaluate_core_iter_into_iter_intrinsic,
    evaluate_core_iter_next_intrinsic, evaluate_core_option_is_some_intrinsic,
    evaluate_core_option_unwrap_intrinsic,
};
use crate::value::{RuntimeISizeValue, RuntimeUSizeValue};
use crate::value::{
    evaluate_core_iter_collect_intrinsic, evaluate_core_range_intrinsic,
    evaluate_std_float_intrinsic,
};

mod calls;
mod function;
mod sequence;

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
        pure_backend: &mut impl RuntimeCallBackend,
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
        pure_backend: &mut impl RuntimeCallBackend,
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
        pure_backend: &mut impl RuntimeCallBackend,
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
            | RuntimeExpr::Range { .. }
            | RuntimeExpr::Record(_)
            | RuntimeExpr::Variant { .. }
            | RuntimeExpr::Field { .. }
            | RuntimeExpr::ProjectTuple { .. }
            | RuntimeExpr::ProjectRecord { .. }
            | RuntimeExpr::AssignField { .. } => self.evaluate_data_expr(expr, pure_backend),
            RuntimeExpr::SpreadArg(_) => Err(RuntimeEvalError::SpreadOutsideCall),
            RuntimeExpr::Call { callee, args } => {
                self.evaluate_call_expr(callee, args, pure_backend)
            }
            RuntimeExpr::Function { params, body } => Ok(self.evaluate_function_expr(params, body)),
            RuntimeExpr::Apply { callee, args } => {
                self.evaluate_apply_expr(callee, args, pure_backend)
            }
            RuntimeExpr::TraitCall {
                callable,
                receiver,
                receiver_mode,
                args,
            } => self
                .evaluate_trait_method_call(*callable, *receiver_mode, receiver, args, pure_backend)
                .map(|outcome| outcome.value),
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
            RuntimeExpr::Filter {
                source,
                param,
                body,
            } => self.evaluate_filter_expr(source, param, body, pure_backend),
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
        pure_backend: &mut impl RuntimeCallBackend,
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
            RuntimeExpr::Range {
                start,
                end,
                inclusive,
            } => {
                self.evaluate_range_expr(start.as_deref(), end.as_deref(), *inclusive, pure_backend)
            }
            RuntimeExpr::Record(fields) => self.evaluate_record_expr(fields, pure_backend),
            RuntimeExpr::Variant {
                owner,
                ordinal,
                name,
                payload,
            } => {
                if owner
                    .variant_case(*ordinal)
                    .is_none_or(|case| case.name != *name)
                {
                    return Err(RuntimeEvalError::PatternMismatch(format!(
                        "variant owner {owner:?} case {ordinal} `{name}`"
                    )));
                }
                let owner = owner.variant_identity().ok_or_else(|| {
                    RuntimeEvalError::PatternMismatch(format!(
                        "non-variant checked owner {owner:?}"
                    ))
                })?;
                Ok(RuntimeValue::Variant {
                    owner,
                    ordinal: *ordinal,
                    name: name.clone(),
                    payload: payload
                        .as_ref()
                        .map(|expr| {
                            self.evaluate_expr_with_backend(expr, pure_backend)
                                .map(Box::new)
                        })
                        .transpose()?,
                })
            }
            RuntimeExpr::Field { target, field } => {
                self.evaluate_field_expr(target, field, pure_backend)
            }
            RuntimeExpr::ProjectTuple { target, ordinal } => {
                self.evaluate_project_tuple_expr(target, *ordinal, pure_backend)
            }
            RuntimeExpr::ProjectRecord { target, ordinal } => {
                self.evaluate_project_record_expr(target, *ordinal, pure_backend)
            }
            RuntimeExpr::AssignField {
                target,
                field,
                expr,
                body,
            } => self.evaluate_assign_field_expr(target, field, expr, body, pure_backend),
            _ => unreachable!("data expression helper received non-data expression"),
        }
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
        pure_backend: &mut impl RuntimeCallBackend,
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
        pure_backend: &mut impl RuntimeCallBackend,
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
        pure_backend: &mut impl RuntimeCallBackend,
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

    fn call_i8_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[i8],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[i8]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_i8_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_i8_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_i8_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_i8_batch_outputs = out;
        Ok(result)
    }

    fn call_i16_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[i16],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[i16]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_i16_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_i16_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_i16_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_i16_batch_outputs = out;
        Ok(result)
    }

    fn call_i128_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[i128],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[i128]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_i128_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_i128_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_i128_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_i128_batch_outputs = out;
        Ok(result)
    }

    fn call_u32_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[u32],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[u32]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_u32_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_u32_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_u32_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_u32_batch_outputs = out;
        Ok(result)
    }

    fn call_u8_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[u8],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_u8_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_u8_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_u8_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_u8_batch_outputs = out;
        Ok(result)
    }

    fn call_u16_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[u16],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[u16]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_u16_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_u16_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_u16_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_u16_batch_outputs = out;
        Ok(result)
    }

    fn call_u128_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[u128],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[u128]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_u128_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_u128_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_u128_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_u128_batch_outputs = out;
        Ok(result)
    }

    fn call_u64_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[u64],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[u64]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_u64_batch_outputs);
        out.resize(row_count, 0);
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_u64_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_u64_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_u64_batch_outputs = out;
        Ok(result)
    }

    fn call_f32_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[f32],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[f32]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_f32_batch_outputs);
        out.resize(row_count, f32::from_bits(0));
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_f32_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_f32_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_f32_batch_outputs = out;
        Ok(result)
    }

    fn call_f64_flat_batch_with_outputs<T>(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[f64],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
        map_outputs: impl FnOnce(&[f64]) -> T,
    ) -> Result<T, RuntimeEvalError> {
        let mut out = std::mem::take(&mut self.pure_f64_batch_outputs);
        out.resize(row_count, f64::from_bits(0));
        let helper = &self.plan.pure_helpers[helper_id.0];
        let batch_result = pure_backend.call_f64_flat_batch(helper, flat_inputs, arity, &mut out);
        if let Err(error) = batch_result {
            self.pure_f64_batch_outputs = out;
            return Err(error);
        }
        let result = map_outputs(&out);
        out.clear();
        self.pure_f64_batch_outputs = out;
        Ok(result)
    }

    fn call_i64_flat_batch_sum(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        flat_inputs: &[i64],
        arity: usize,
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<i64, RuntimeEvalError> {
        let helper = &self.plan.pure_helpers[helper_id.0];
        pure_backend.call_i64_flat_batch_sum(helper, flat_inputs, arity, row_count)
    }

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper_id: crate::plan::RuntimePureHelperId,
        row: &[i64],
        row_count: usize,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<i64, RuntimeEvalError> {
        let helper = &self.plan.pure_helpers[helper_id.0];
        pure_backend.call_i64_repeated_flat_batch_sum(helper, row, row_count)
    }

    fn evaluate_record_expr(
        &mut self,
        fields: &[RuntimeFieldExpr],
        pure_backend: &mut impl RuntimeCallBackend,
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
        pure_backend: &mut impl RuntimeCallBackend,
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
            RuntimeValue::Seq(RuntimeSeq::RecordColumns(records)) => records
                .field_by_name(field)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.to_owned(),
                    value: "record sequence".to_owned(),
                }),
            RuntimeValue::EntityRef(id) => {
                Self::entity_ref_field(&id, field).ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.to_owned(),
                    value: "entity reference".to_owned(),
                })
            }
            value => Err(RuntimeEvalError::MissingField {
                field: field.to_owned(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn entity_ref_field(id: &str, field: &str) -> Option<RuntimeValue> {
        Some(match field {
            "id" => RuntimeValue::String(id.to_owned()),
            "family" => RuntimeValue::String(Self::entity_ref_family(id).to_owned()),
            "name" => RuntimeValue::String(Self::entity_ref_name(id).to_owned()),
            _ => return None,
        })
    }

    fn entity_ref_family(id: &str) -> &str {
        id.split_once('.').map_or(id, |(family, _)| family)
    }

    fn entity_ref_name(id: &str) -> &str {
        id.split_once('.').map_or("", |(_, name)| name)
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
                |field| Ok(field.value),
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
        name: &str,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        self.fiber.env.push_scope_with_capacity(1);
        self.fiber.env.set(name.to_owned(), value);
        let result = self.evaluate_expr_with_backend(body, pure_backend);
        self.fiber.env.pop_scope();
        result
    }

    fn evaluate_assign_field_expr(
        &mut self,
        target: &RuntimeExpr,
        field: &str,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let RuntimeExpr::Local(binding) = target else {
            let value = self.evaluate_expr_with_backend(target, pure_backend)?;
            return Err(RuntimeEvalError::InvalidFieldAssignment {
                field: field.to_owned(),
                value: runtime_value_label(&value),
            });
        };
        let value = self.evaluate_expr_with_backend(expr, pure_backend)?;
        self.fiber
            .env
            .set_record_field(binding, field, value)
            .map_err(|target| RuntimeEvalError::InvalidFieldAssignment {
                field: field.to_owned(),
                value: runtime_value_label(&target),
            })?;
        self.evaluate_expr_with_backend(body, pure_backend)
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
        pure_backend: &mut impl RuntimeCallBackend,
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
    ) -> Result<FlowRuntimeId, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::EntityRef(target) | RuntimeValue::String(target) => self
                .plan
                .resolve_flow_target_value(&target)
                .map_err(|error| RuntimeEvalError::InvalidEntityTarget {
                    target,
                    reason: error.to_string(),
                }),
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
        output.diagnostics.push(RuntimeDiagnostic::new(message));
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
    expr_returns_integer(&helper.expr, &mut int_names)
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
    expr_returns_integer(&helper.expr, &mut int_names)
}

pub(super) fn pure_helper_has_u32_call_shape(helper: &crate::plan::RuntimePureHelper) -> bool {
    pure_helper_has_exact_int_call_shape::<u32>(helper)
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
    expr_returns_integer(&helper.expr, &mut int_names)
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

fn expr_returns_integer<'a>(expr: &'a RuntimeExpr, int_names: &mut Vec<&'a str>) -> bool {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(_) | RuntimeValue::UInt(_)) => true,
        RuntimeExpr::Local(name) => int_names.contains(&name.as_str()),
        RuntimeExpr::Let { name, expr, body } => {
            if !expr_returns_integer(expr, int_names) {
                return false;
            }
            let original_len = int_names.len();
            int_names.push(name.as_str());
            let returns_integer = expr_returns_integer(body, int_names);
            int_names.truncate(original_len);
            returns_integer
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) =>
        {
            args.iter().all(|arg| expr_returns_integer(arg, int_names))
        }
        RuntimeExpr::Unary {
            op: crate::value::RuntimeUnaryOp::Neg,
            expr,
        } => expr_returns_integer(expr, int_names),
        RuntimeExpr::Binary {
            lhs,
            op:
                RuntimeBinaryOp::Add
                | RuntimeBinaryOp::Sub
                | RuntimeBinaryOp::Mul
                | RuntimeBinaryOp::Div,
            rhs,
        } => expr_returns_integer(lhs, int_names) && expr_returns_integer(rhs, int_names),
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_returns_bool(condition, int_names)
                && expr_returns_integer(then_expr, int_names)
                && expr_returns_integer(else_expr, int_names)
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
        } => expr_returns_integer(lhs, int_names) && expr_returns_integer(rhs, int_names),
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

fn evaluate_core_iterator_intrinsic(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Option<RuntimeValue> {
    match (intrinsic, args) {
        (RuntimeIntrinsic::CoreIterCollect, [value]) => Some(
            evaluate_core_iter_collect_intrinsic(value.clone()).unwrap_or_else(|error| {
                RuntimeValue::String(format!("core.iter.collect({error})"))
            }),
        ),
        (RuntimeIntrinsic::CoreIterIntoIter, [value, evidence]) => Some(
            evaluate_core_iter_into_iter_intrinsic(value.clone(), evidence).unwrap_or_else(
                |error| RuntimeValue::String(format!("core.iter.into_iter({error})")),
            ),
        ),
        (RuntimeIntrinsic::CoreIterNext, [value]) => Some(
            evaluate_core_iter_next_intrinsic(value.clone())
                .unwrap_or_else(|error| RuntimeValue::String(format!("core.iter.next({error})"))),
        ),
        (RuntimeIntrinsic::CoreOptionIsSome, [value]) => Some(
            evaluate_core_option_is_some_intrinsic(value).unwrap_or_else(|error| {
                RuntimeValue::String(format!("core.option.is_some({error})"))
            }),
        ),
        (RuntimeIntrinsic::CoreOptionUnwrap, [value]) => Some(
            evaluate_core_option_unwrap_intrinsic(value.clone()).unwrap_or_else(|error| {
                RuntimeValue::String(format!("core.option.unwrap({error})"))
            }),
        ),
        _ => None,
    }
}

pub(crate) fn evaluate_runtime_call(
    callee: &RuntimeCallTarget,
    args: &[RuntimeValue],
    pure_backend: &mut impl RuntimeCallBackend,
) -> RuntimeValue {
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Ok(Some(value)) = evaluate_std_float_intrinsic(intrinsic, args)
    {
        return value;
    }
    if let Some(intrinsic) = callee.as_intrinsic()
        && let Some(value) = evaluate_core_iterator_intrinsic(intrinsic, args)
    {
        return value;
    }
    match (callee.as_intrinsic(), args) {
        (Some(RuntimeIntrinsic::Add), [RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)]) => {
            evaluate_binary(
                RuntimeValue::Int(*lhs),
                RuntimeBinaryOp::Add,
                RuntimeValue::Int(*rhs),
            )
            .unwrap_or_else(|_| RuntimeValue::String("add(<unsupported>)".to_owned()))
        }
        (Some(RuntimeIntrinsic::CoreRange), _) => evaluate_core_range_intrinsic(args)
            .unwrap_or_else(|error| RuntimeValue::String(format!("core.range({error})"))),
        (
            Some(RuntimeIntrinsic::MathMatmulF32),
            [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
        ) => pure_backend.call_math_matmul_f32(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.matmul_f32({error})")),
            RuntimeValue::matrix_f32,
        ),
        (
            Some(RuntimeIntrinsic::MathMatrixAddF32),
            [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
        ) => pure_backend.call_math_matrix_add_f32(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.matrix_add_f32({error})")),
            RuntimeValue::matrix_f32,
        ),
        (
            Some(RuntimeIntrinsic::MathTensorAddF32),
            [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
        ) => pure_backend.call_math_tensor_add_f32(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.tensor_add_f32({error})")),
            RuntimeValue::tensor_f32,
        ),
        (
            Some(RuntimeIntrinsic::MathMatmulF64),
            [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
        ) => pure_backend.call_math_matmul_f64(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.matmul_f64({error})")),
            RuntimeValue::matrix_f64,
        ),
        (
            Some(RuntimeIntrinsic::MathMatrixAddF64),
            [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
        ) => pure_backend.call_math_matrix_add_f64(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.matrix_add_f64({error})")),
            RuntimeValue::matrix_f64,
        ),
        (
            Some(RuntimeIntrinsic::MathTensorAddF64),
            [RuntimeValue::TensorF64(lhs), RuntimeValue::TensorF64(rhs)],
        ) => pure_backend.call_math_tensor_add_f64(lhs, rhs).map_or_else(
            |error| RuntimeValue::String(format!("math.tensor_add_f64({error})")),
            RuntimeValue::tensor_f64,
        ),
        (
            Some(
                intrinsic @ (RuntimeIntrinsic::PathSave
                | RuntimeIntrinsic::PathAsset
                | RuntimeIntrinsic::PathTemp
                | RuntimeIntrinsic::PathExport),
            ),
            [RuntimeValue::String(path)],
        ) => {
            let space = intrinsic.path_space().unwrap_or(intrinsic.as_label());
            RuntimeValue::String(format!("{space}:{path}"))
        }
        _ => pure_backend.call_external(callee, args).map_or_else(
            || {
                RuntimeValue::String(format!(
                    "{}({})",
                    callee.as_label(),
                    args.iter()
                        .map(runtime_value_label)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            },
            |result| {
                result.unwrap_or_else(|error| {
                    RuntimeValue::String(format!("{}({error})", callee.as_label()))
                })
            },
        ),
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
        (RuntimeValue::Seq(seq), "len", []) => RuntimeValue::from_collection_len(seq.len()),
        (RuntimeValue::Seq(seq), "contains", [needle]) => {
            RuntimeValue::Bool(seq.into_values().iter().any(|item| item == needle))
        }
        (RuntimeValue::Seq(seq), "require_role", [RuntimeValue::String(role)]) => seq
            .into_values()
            .into_iter()
            .find(|item| runtime_record_string_field(item, "role").as_deref() == Some(role))
            .unwrap_or(RuntimeValue::Unit),
        (RuntimeValue::Seq(seq), "__index", [index]) => seq.value_at_runtime_index(index),
        (RuntimeValue::Tuple(items), "len", []) => RuntimeValue::from_collection_len(items.len()),
        (RuntimeValue::Tuple(items), "contains", [needle]) => {
            RuntimeValue::Bool(items.iter().any(|item| item == needle))
        }
        (RuntimeValue::Tuple(items), "__index", [index]) => index
            .to_collection_index()
            .and_then(|index| items.get(index).cloned())
            .unwrap_or(RuntimeValue::Unit),
        (
            RuntimeValue::Record(fields),
            "get",
            [RuntimeValue::String(key) | RuntimeValue::EntityRef(key)],
        ) => fields
            .iter()
            .find(|field| field.name == *key)
            .map_or(RuntimeValue::Unit, |field| field.value.clone()),
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

fn runtime_record_string_field(value: &RuntimeValue, field: &str) -> Option<String> {
    let RuntimeValue::Record(fields) = value else {
        return None;
    };
    fields
        .iter()
        .find(|candidate| candidate.name == field)
        .and_then(|candidate| match &candidate.value {
            RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => Some(value.clone()),
            _ => None,
        })
}
