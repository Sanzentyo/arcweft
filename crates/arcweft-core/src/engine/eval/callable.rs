//! Native expression entry into the program's callable-state authority.

#[cfg(test)]
#[path = "function/tests.rs"]
mod tests;

use std::sync::Arc;

use crate::plan::{RuntimeFunctionInputSource, RuntimeFunctionSiteBody};
use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallArgument, RuntimeCallArgumentMode, RuntimeCallableApplication,
    RuntimeCallableBodyReference, RuntimeCallableValue, RuntimeCallableValueError, RuntimeValue,
};

use super::{
    Engine, RuntimeCallBackend, RuntimeEvalError, RuntimeExpr, match_runtime_pattern,
    runtime_value_label, spread_runtime_values,
};

impl Engine {
    pub(super) fn evaluate_specialize_callable_expr(
        &mut self,
        value: &RuntimeExpr,
        specialization: crate::runtime_id::RuntimeCallableSpecializationId,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(value, backend)?;
        let RuntimeValue::Callable(callable) = value else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &value,
            )));
        };
        callable
            .specialize(
                &RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
                specialization,
            )
            .map(RuntimeValue::Callable)
            .map_err(Into::into)
    }

    pub(in crate::engine) fn evaluate_dialogue_site(
        &mut self,
        site: &crate::plan::RuntimeDialogueValueSite,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let captures = site
            .captures()
            .iter()
            .map(|capture| self.evaluate_expr_with_backend(capture, backend))
            .collect::<Result<Vec<_>, _>>()?;
        self.evaluate_function_site(site.function(), &captures, &[], backend)
    }

    pub(in crate::engine) fn evaluate_callable_expr(
        &mut self,
        state: RuntimeCallableStateId,
        captures: &[RuntimeExpr],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let captures = captures
            .iter()
            .map(|capture| self.evaluate_expr_with_backend(capture, backend))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::Callable(RuntimeCallableValue::try_new(
            RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
            state,
            captures,
        )?))
    }

    pub(super) fn evaluate_apply_expr(
        &mut self,
        callee: &RuntimeExpr,
        args: &[RuntimeCallArgument],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let callee = self.evaluate_expr_with_backend(callee, backend)?;
        let args = self.evaluate_function_call_args(args, backend)?;
        let RuntimeValue::Callable(callable) = callee else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &callee,
            )));
        };
        self.apply_runtime_function(&callable, &args, backend)
    }

    pub(in crate::engine) fn evaluate_function_call_args(
        &mut self,
        args: &[RuntimeCallArgument],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut materialized = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr_with_backend(argument.value(), backend)?;
            let values = match argument.mode() {
                RuntimeCallArgumentMode::Value => vec![value],
                RuntimeCallArgumentMode::Spread => spread_runtime_values(value)?,
            };
            materialized.push((argument.abi_position(), values));
        }
        materialized.sort_by_key(|(position, _)| *position);
        Ok(materialized
            .into_iter()
            .flat_map(|(_, values)| values)
            .collect())
    }

    pub(crate) fn apply_runtime_function(
        &mut self,
        callable: &RuntimeCallableValue,
        args: &[RuntimeValue],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        callable.validate_for_owner(&RuntimeProgramOwner::Plan(Arc::clone(&self.plan)))?;
        if args.len() < callable.remaining_arity()? {
            return Ok(RuntimeValue::Callable(callable.try_bind_prefix(args)?));
        }
        let (arguments, attached) = callable.materialize_abi_arguments(args)?;
        let mut application = callable.prepare_group(&arguments, attached)?;
        loop {
            match application {
                RuntimeCallableApplication::Complete(value) => return Ok(value),
                RuntimeCallableApplication::Invoke(invocation) => {
                    let RuntimeCallableBodyReference::Plan(site) = invocation.body else {
                        return Err(RuntimeCallableValueError::ForeignProgram.into());
                    };
                    return self.evaluate_function_site(
                        site,
                        &invocation.captures,
                        &invocation.arguments,
                        backend,
                    );
                }
                RuntimeCallableApplication::AttachedDefault(invocation) => {
                    let RuntimeCallableBodyReference::Plan(site) = invocation.body else {
                        return Err(RuntimeCallableValueError::ForeignProgram.into());
                    };
                    let value = self.evaluate_function_site(
                        site,
                        &invocation.captures,
                        &invocation.arguments,
                        backend,
                    )?;
                    application = callable.complete_group_default(&arguments, value)?;
                }
            }
        }
    }

    pub(in crate::engine) fn evaluate_function_site(
        &mut self,
        site: RuntimeFunctionSiteId,
        captures: &[RuntimeValue],
        arguments: &[RuntimeValue],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = Arc::clone(&self.plan);
        let declaration = plan.validate_function_site_inputs(site, captures, arguments)?;
        let RuntimeFunctionSiteBody::Expression(body) = declaration.body() else {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "structured.function".to_owned(),
                reason: "an executable runtime function requires function-call control transfer"
                    .to_owned(),
            });
        };
        self.fiber
            .env
            .push_scope_with_capacity(declaration.inputs().len());
        let value = (|| {
            for input in declaration.inputs() {
                let value = match input.source() {
                    RuntimeFunctionInputSource::Capture { position } => {
                        &captures[position as usize]
                    }
                    RuntimeFunctionInputSource::Parameter { position } => {
                        &arguments[position as usize]
                    }
                };
                let bindings = match_runtime_pattern(&plan, input.pattern(), value)?
                    .ok_or_else(|| RuntimeEvalError::PatternMismatch(runtime_value_label(value)))?;
                self.fiber.env.set_ref(input.input_local(), value);
                self.fiber.env.bind_all(bindings);
            }
            self.evaluate_expr_with_backend(body, backend)
        })();
        self.fiber.env.pop_scope();
        value
    }
}
