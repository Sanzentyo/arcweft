use super::{
    Engine, FlowFiberStatus, RuntimeBinding, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr,
    RuntimeExprMatchArm, RuntimeFieldValue, RuntimeMatchArm, RuntimeMatchSelection, RuntimePattern,
    RuntimeStepOutput, RuntimeValue, evaluate_binary, evaluate_unary, match_runtime_pattern,
    runtime_value_label,
};
use crate::pure::{RuntimeI64Args, RuntimePureCallBackend, VmRuntimePureCallBackend};
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
            let matched = self.with_temp_bindings(bindings.clone(), |this| {
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
                && !self.with_temp_bindings(bindings.clone(), |this| {
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
            let flat_inputs = self.collect_i64_pure_batch_inputs(items, arity, pure_backend)?;
            let helper = &self.plan.pure_helpers[helper_id.0];
            let mut out = vec![0; items.len()];
            pure_backend.call_i64_flat_batch(helper, &flat_inputs, arity, &mut out)?;
            return Ok(RuntimeValue::BracketSeq(
                out.into_iter().map(RuntimeValue::Int).collect(),
            ));
        }
        items
            .iter()
            .map(|item| self.evaluate_expr_with_backend(item, pure_backend))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeValue::BracketSeq)
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
        let helper = &self.plan.pure_helpers[first_helper.0];
        if !pure_helper_returns_i64(helper) {
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
    ) -> Result<Vec<i64>, RuntimeEvalError> {
        let mut flat_inputs = Vec::with_capacity(items.len().saturating_mul(arity));
        for item in items {
            let RuntimeExpr::PureCall { args, .. } = item else {
                unreachable!("i64 pure batch shape checked before row collection");
            };
            for arg in args.iter().take(arity) {
                flat_inputs.push(self.evaluate_i64_arg_with_backend(arg, pure_backend)?);
            }
        }
        Ok(flat_inputs)
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
        if args.len() <= RuntimeI64Args::MAX
            && !args
                .iter()
                .any(|arg| matches!(arg, RuntimeExpr::SpreadArg(_)))
        {
            let mut values = [0_i64; RuntimeI64Args::MAX];
            for (index, arg) in args.iter().enumerate() {
                values[index] = self.evaluate_i64_arg_with_backend(arg, pure_backend)?;
            }
            let helper = &self.plan.pure_helpers[helper_id.0];
            if let Some(value) =
                pure_backend.call_i64(helper, RuntimeI64Args::new(values, args.len()))?
            {
                return Ok(RuntimeValue::Int(value));
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
        self.fiber.env.push_scope();
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
            self.with_temp_bindings(bindings.clone(), |this| {
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
                && !self.with_temp_bindings(bindings.clone(), |this| {
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

    pub(super) fn with_temp_bindings<T>(
        &mut self,
        bindings: impl IntoIterator<Item = RuntimeBinding>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.fiber.env.push_scope();
        self.fiber.env.bind_all(bindings);
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

fn pure_helper_returns_i64(helper: &crate::plan::RuntimePureHelper) -> bool {
    let mut int_names = helper
        .input_names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    expr_returns_i64(&helper.expr, &mut int_names)
}

fn expr_returns_i64<'a>(expr: &'a RuntimeExpr, int_names: &mut Vec<&'a str>) -> bool {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(_)) => true,
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
    match value {
        RuntimeValue::Tuple(items) | RuntimeValue::BracketSeq(items) => Ok(items),
        value => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
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
