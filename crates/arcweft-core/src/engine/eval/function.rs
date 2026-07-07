use super::{
    Engine, RuntimeCallBackend, RuntimeEvalError, RuntimeExpr, RuntimeFunctionValue, RuntimeValue,
    runtime_value_label,
};

impl Engine {
    pub(super) fn evaluate_function_expr(
        &self,
        params: &[String],
        body: &RuntimeExpr,
    ) -> RuntimeValue {
        RuntimeValue::Function(RuntimeFunctionValue::new(
            params.to_vec(),
            body.clone(),
            self.fiber.env.bindings_snapshot(),
        ))
    }

    pub(super) fn evaluate_apply_expr(
        &mut self,
        callee: &RuntimeExpr,
        args: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let callee = self.evaluate_expr_with_backend(callee, pure_backend)?;
        let args = self.evaluate_call_args(args, pure_backend)?;
        match callee {
            RuntimeValue::Function(function) => {
                self.apply_runtime_function(&function, &args, pure_backend)
            }
            value => Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &value,
            ))),
        }
    }

    fn apply_runtime_function(
        &mut self,
        function: &RuntimeFunctionValue,
        args: &[RuntimeValue],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() < function.arity() {
            return Ok(RuntimeValue::Function(function.partially_apply(args)));
        }

        let (call_args, remaining_args) = args.split_at(function.arity());
        let value = self.call_runtime_function(function, call_args, pure_backend)?;
        if remaining_args.is_empty() {
            return Ok(value);
        }
        match value {
            RuntimeValue::Function(next) => {
                self.apply_runtime_function(&next, remaining_args, pure_backend)
            }
            _ => Err(RuntimeEvalError::FunctionArgumentCount {
                expected: function.arity(),
                found: args.len(),
            }),
        }
    }

    fn call_runtime_function(
        &mut self,
        function: &RuntimeFunctionValue,
        args: &[RuntimeValue],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.fiber
            .env
            .push_scope_with_capacity(function.captures.len() + args.len());
        self.fiber.env.bind_all_ref(&function.captures);
        for (param, value) in function.params.iter().zip(args) {
            self.fiber.env.set_ref(param, value);
        }
        let Some(body) = function.expr_body() else {
            self.fiber.env.pop_scope();
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "awbc.function".to_owned(),
                reason: "structured runtime cannot evaluate an AWBC function body".to_owned(),
            });
        };
        let result = self.evaluate_expr_with_backend(body, pure_backend);
        self.fiber.env.pop_scope();
        result
    }
}
