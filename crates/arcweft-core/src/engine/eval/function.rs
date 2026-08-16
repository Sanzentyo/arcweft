use std::sync::Arc;

use crate::runtime_id::RuntimeFunctionSiteId;
use crate::value::{
    RuntimeCallArgument, RuntimeCallArgumentMode, RuntimeFunctionApplyError, RuntimeFunctionValue,
};

use super::{
    Engine, RuntimeCallBackend, RuntimeEvalError, RuntimeExpr, RuntimeValue, runtime_value_label,
    spread_runtime_values,
};

impl Engine {
    pub(in crate::engine) fn evaluate_dialogue_site(
        &mut self,
        site: RuntimeFunctionSiteId,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let function = self.evaluate_function_expr(site)?;
        let RuntimeValue::Function(function) = function else {
            unreachable!("structured function-site construction returned a non-function")
        };
        self.apply_runtime_function(&function, &[], pure_backend)
    }

    pub(super) fn evaluate_function_expr(
        &self,
        site: RuntimeFunctionSiteId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let site_declaration = self
            .plan
            .function_sites()
            .get(site)
            .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
        let capture_values = site_declaration
            .captures()
            .iter()
            .map(|&local| {
                self.fiber
                    .env
                    .get_cloned(local)
                    .ok_or(RuntimeEvalError::UnboundStructuredCapture { site, local })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::Function(RuntimeFunctionValue::capture_site(
            Arc::clone(&self.plan),
            site,
            capture_values,
        )?))
    }

    pub(super) fn evaluate_apply_expr(
        &mut self,
        callee: &RuntimeExpr,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let callee = self.evaluate_expr_with_backend(callee, pure_backend)?;
        let args = self.evaluate_function_call_args(args, pure_backend)?;
        match callee {
            RuntimeValue::Function(function) => {
                self.apply_runtime_function(&function, &args, pure_backend)
            }
            value => Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &value,
            ))),
        }
    }

    fn evaluate_function_call_args(
        &mut self,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut values = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr_with_backend(argument.value(), pure_backend)?;
            match argument.mode() {
                RuntimeCallArgumentMode::Value => values.push(value),
                RuntimeCallArgumentMode::Spread => values.extend(spread_runtime_values(value)?),
            }
        }
        Ok(values)
    }

    fn apply_runtime_function(
        &mut self,
        function: &RuntimeFunctionValue,
        args: &[RuntimeValue],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let Some(closure) = function.as_structured() else {
            return Err(structured_awbc_function_error());
        };
        if !Arc::ptr_eq(&self.plan, closure.plan()) {
            return Err(RuntimeEvalError::ForeignStructuredFunction {
                site: closure.site(),
            });
        }

        let remaining = function.remaining_arity()?;
        if args.len() < remaining {
            return Ok(RuntimeValue::Function(function.try_bind_prefix(args)?));
        }

        let (call_args, remaining_args) = args.split_at(remaining);
        let value = self.call_runtime_function(function, call_args, pure_backend)?;
        if remaining_args.is_empty() {
            return Ok(value);
        }
        match value {
            RuntimeValue::Function(next) => {
                self.apply_runtime_function(&next, remaining_args, pure_backend)
            }
            _ => Err(RuntimeEvalError::FunctionArgumentCount {
                expected: remaining,
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
        let Some(closure) = function.as_structured() else {
            return Err(structured_awbc_function_error());
        };
        if !Arc::ptr_eq(&self.plan, closure.plan()) {
            return Err(RuntimeEvalError::ForeignStructuredFunction {
                site: closure.site(),
            });
        }

        let remaining = function.remaining_arity()?;
        if args.len() != remaining {
            return Err(RuntimeEvalError::FunctionArgumentCount {
                expected: remaining,
                found: args.len(),
            });
        }
        function.validate_bind_prefix(args)?;

        let plan = Arc::clone(&self.plan);
        let site = closure.site();
        let site_declaration = plan
            .function_sites()
            .get(site)
            .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
        self.fiber.env.push_scope_with_capacity(
            site_declaration.captures().len() + site_declaration.params().len(),
        );
        for (&local, value) in site_declaration
            .captures()
            .iter()
            .zip(closure.capture_values())
        {
            self.fiber.env.set_ref(local, value);
        }
        let bound_count = closure.bound_args().len();
        for (&local, value) in site_declaration.params()[..bound_count]
            .iter()
            .zip(closure.bound_args())
        {
            self.fiber.env.set_ref(local, value);
        }
        for (&local, value) in site_declaration.params()[bound_count..].iter().zip(args) {
            self.fiber.env.set_ref(local, value);
        }
        let result = self.evaluate_expr_with_backend(site_declaration.body(), pure_backend);
        self.fiber.env.pop_scope();
        result
    }
}

fn structured_awbc_function_error() -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: "awbc.function".to_owned(),
        reason: "structured runtime cannot evaluate an AWBC function body".to_owned(),
    }
}
