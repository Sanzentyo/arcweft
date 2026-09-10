use std::sync::Arc;

#[cfg(test)]
mod tests;

use crate::plan::{RuntimeFunctionInputSource, RuntimeFunctionSiteBody};
use crate::runtime_id::RuntimeFunctionSiteId;
use crate::value::{
    RuntimeCallArgument, RuntimeCallArgumentMode, RuntimeFunctionApplyError, RuntimeFunctionValue,
    RuntimeValue,
};

use super::{
    Engine, RuntimeCallBackend, RuntimeEvalError, RuntimeExpr, match_runtime_pattern,
    runtime_value_label, spread_runtime_values,
};

impl Engine {
    pub(in crate::engine) fn evaluate_dialogue_site(
        &mut self,
        site: &crate::plan::RuntimeDialogueValueSite,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let function =
            self.evaluate_function_expr(site.function(), site.captures(), pure_backend)?;
        let RuntimeValue::Function(function) = function else {
            unreachable!("structured function-site construction returned a non-function")
        };
        self.apply_runtime_function(&function, &[], pure_backend)
    }

    pub(in crate::engine) fn evaluate_function_expr(
        &mut self,
        site: RuntimeFunctionSiteId,
        captures: &[RuntimeExpr],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let capture_inputs = self
            .plan
            .function_sites()
            .get(site)
            .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?
            .capture_inputs()
            .count();
        if captures.len() != capture_inputs {
            return Err(RuntimeEvalError::FunctionApply(
                RuntimeFunctionApplyError::CaptureCountMismatch {
                    site,
                    expected: capture_inputs,
                    actual: captures.len(),
                },
            ));
        }
        let capture_values = captures
            .iter()
            .map(|capture| self.evaluate_expr_with_backend(capture, pure_backend))
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

    pub(in crate::engine) fn evaluate_function_call_args(
        &mut self,
        args: &[RuntimeCallArgument],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut materialized = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr_with_backend(argument.value(), pure_backend)?;
            let values = match argument.mode() {
                RuntimeCallArgumentMode::Value => vec![value],
                RuntimeCallArgumentMode::Spread => spread_runtime_values(value)?,
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

    pub(crate) fn apply_runtime_function(
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

        self.call_runtime_function(function, args, pure_backend)
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
        let body = match site_declaration.body() {
            RuntimeFunctionSiteBody::Expression(body) => body,
            RuntimeFunctionSiteBody::Executable(_) => {
                return Err(structured_executable_apply_error());
            }
        };
        self.fiber
            .env
            .push_scope_with_capacity(site_declaration.inputs().len());
        let mut parameter_values = closure.bound_args().to_vec();
        parameter_values.extend_from_slice(args);
        for input in site_declaration.inputs() {
            let value = match input.source() {
                RuntimeFunctionInputSource::Capture { position } => closure
                    .capture_values()
                    .get(usize::try_from(position).map_err(|_| {
                        RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site }
                    })?)
                    .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })?,
                RuntimeFunctionInputSource::Parameter { position } => parameter_values
                    .get(usize::try_from(position).map_err(|_| {
                        RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site }
                    })?)
                    .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })?,
            };
            self.fiber.env.set_ref(input.input_local(), value);
            let bindings = match match_runtime_pattern(&plan, input.pattern(), value) {
                Ok(Some(bindings)) => bindings,
                Ok(None) => {
                    self.fiber.env.pop_scope();
                    return Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
                        value,
                    )));
                }
                Err(error) => {
                    self.fiber.env.pop_scope();
                    return Err(error.into());
                }
            };
            self.fiber.env.bind_all(bindings);
        }
        let result = self.evaluate_expr_with_backend(body, pure_backend);
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

fn structured_executable_apply_error() -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: "structured.function".to_owned(),
        reason: "an executable runtime function requires function-call control transfer".to_owned(),
    }
}
