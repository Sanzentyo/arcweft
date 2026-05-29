use super::{
    Engine, FlowFiberStatus, RuntimeBinding, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr,
    RuntimeExprMatchArm, RuntimeFieldValue, RuntimeMatchArm, RuntimeMatchSelection, RuntimePattern,
    RuntimeStepOutput, RuntimeValue, evaluate_binary, evaluate_unary, match_runtime_pattern,
    runtime_value_label,
};

impl Engine {
    pub(super) fn evaluate_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut RuntimeStepOutput,
    ) {
        match self.evaluate_expr(expr).and_then(|value| {
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

    pub(super) fn evaluate_if_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
    ) -> Result<Option<Vec<RuntimeBinding>>, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        let Some(bindings) = match_runtime_pattern(pattern, &value)? else {
            return Ok(None);
        };
        if let Some(guard) = guard {
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings.clone());
            let matched = self.evaluate_bool(guard);
            self.fiber.env = previous;
            let matched = matched?;
            if !matched {
                return Ok(None);
            }
        }
        Ok(Some(bindings))
    }

    pub(super) fn evaluate_match(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeMatchArm],
    ) -> Result<RuntimeMatchSelection, RuntimeEvalError> {
        let value = self.evaluate_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings.clone());
            if let Some(guard) = arm.guard.as_ref()
                && !match self.evaluate_bool(guard) {
                    Ok(matched) => matched,
                    Err(error) => {
                        self.fiber.env = previous;
                        return Err(error);
                    }
                }
            {
                self.fiber.env = previous;
                continue;
            }
            self.fiber.env = previous;
            return Ok(Some((bindings, arm.ops.clone())));
        }
        Ok(None)
    }

    pub(super) fn evaluate_expr(
        &mut self,
        expr: &RuntimeExpr,
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
            RuntimeExpr::Let { name, expr, body } => self.evaluate_let_expr(name, expr, body),
            RuntimeExpr::Tuple(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Tuple),
            RuntimeExpr::BracketSeq(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::BracketSeq),
            RuntimeExpr::Record(fields) => fields
                .iter()
                .map(|field| {
                    Ok(RuntimeFieldValue {
                        name: field.name.clone(),
                        value: self.evaluate_expr(&field.value)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Record),
            RuntimeExpr::Variant {
                path,
                name,
                payload,
            } => Ok(RuntimeValue::Variant {
                path: path.clone(),
                name: name.clone(),
                payload: payload
                    .as_ref()
                    .map(|expr| self.evaluate_expr(expr).map(Box::new))
                    .transpose()?,
            }),
            RuntimeExpr::Field { target, field } => {
                let value = self.evaluate_expr(target)?;
                match value {
                    RuntimeValue::Record(fields) => fields
                        .into_iter()
                        .find(|candidate| candidate.name == *field)
                        .map(|field| field.value)
                        .ok_or_else(|| RuntimeEvalError::MissingField {
                            field: field.clone(),
                            value: "record".to_owned(),
                        }),
                    value => Err(RuntimeEvalError::MissingField {
                        field: field.clone(),
                        value: runtime_value_label(&value),
                    }),
                }
            }
            RuntimeExpr::SpreadArg(_) => Err(RuntimeEvalError::SpreadOutsideCall),
            RuntimeExpr::Call { callee, args } => self.evaluate_call_expr(callee, args),
            RuntimeExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.evaluate_method_call_expr(receiver, method, args),
            RuntimeExpr::Unary { op, expr } => {
                let value = self.evaluate_expr(expr)?;
                evaluate_unary(*op, value)
            }
            RuntimeExpr::Binary { lhs, op, rhs } => {
                let lhs = self.evaluate_expr(lhs)?;
                let rhs = self.evaluate_expr(rhs)?;
                evaluate_binary(lhs, *op, rhs)
            }
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.evaluate_bool(condition)? {
                    self.evaluate_expr(then_expr)
                } else {
                    self.evaluate_expr(else_expr)
                }
            }
            RuntimeExpr::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => self.evaluate_if_let_expr(pattern, expr, guard.as_deref(), then_expr, else_expr),
            RuntimeExpr::Match { scrutinee, arms } => self.evaluate_match_expr(scrutinee, arms),
        }
    }

    fn evaluate_call_expr(
        &mut self,
        callee: &str,
        args: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let args = self.evaluate_call_args(args)?;
        Ok(evaluate_runtime_call(callee, &args))
    }

    fn evaluate_method_call_expr(
        &mut self,
        receiver: &RuntimeExpr,
        method: &str,
        args: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let receiver = self.evaluate_expr(receiver)?;
        let args = self.evaluate_call_args(args)?;
        Ok(evaluate_runtime_method_call(receiver, method, &args))
    }

    fn evaluate_call_args(
        &mut self,
        args: &[RuntimeExpr],
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut values = Vec::new();
        for arg in args {
            match arg {
                RuntimeExpr::SpreadArg(expr) => {
                    let spread = self.evaluate_expr(expr)?;
                    values.extend(spread_runtime_values(spread)?);
                }
                expr => values.push(self.evaluate_expr(expr)?),
            }
        }
        Ok(values)
    }

    fn evaluate_let_expr(
        &mut self,
        name: &str,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        self.fiber.env.push_scope();
        self.fiber.env.set(name.to_owned(), value);
        let result = self.evaluate_expr(body);
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
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        let Some(bindings) = match_runtime_pattern(pattern, &value)? else {
            return self.evaluate_expr(else_expr);
        };
        let previous = self.fiber.env.clone();
        self.fiber.env.bind_all(bindings);
        match guard.map_or(Ok(true), |guard| self.evaluate_bool(guard)) {
            Ok(true) => {
                let result = self.evaluate_expr(then_expr);
                self.fiber.env = previous;
                result
            }
            Ok(false) => {
                self.fiber.env = previous;
                self.evaluate_expr(else_expr)
            }
            Err(error) => {
                self.fiber.env = previous;
                Err(error)
            }
        }
    }

    pub(super) fn evaluate_match_expr(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeExprMatchArm],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings);
            if let Some(guard) = arm.guard.as_ref()
                && !match self.evaluate_bool(guard) {
                    Ok(matched) => matched,
                    Err(error) => {
                        self.fiber.env = previous;
                        return Err(error);
                    }
                }
            {
                self.fiber.env = previous;
                continue;
            }
            let result = self.evaluate_expr(&arm.value);
            self.fiber.env = previous;
            return result;
        }
        Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
            &value,
        )))
    }

    pub(super) fn evaluate_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(runtime_value_label(&value))),
        }
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
