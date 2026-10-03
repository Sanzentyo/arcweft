//! Native expression entry into the program's callable-state authority.

#[cfg(test)]
#[path = "function/tests.rs"]
mod tests;

use std::sync::Arc;

use crate::pattern::match_runtime_pattern_owned;
use crate::plan::{RuntimeFunctionInputSource, RuntimeFunctionSiteBody};
use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallArgument, RuntimeCallArgumentMode, RuntimeCallableApplication,
    RuntimeCallableBodyReference, RuntimeCallableValue, RuntimeCallableValueError, RuntimeValue,
};

use super::{
    Engine, RuntimeCallBackend, RuntimeEvalError, RuntimeExpr, runtime_value_label,
    spread_runtime_values,
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
        self.evaluate_function_site(site.function(), captures, Vec::new(), backend)
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
        self.apply_runtime_function(callable, args, backend)
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
        callable: RuntimeCallableValue,
        args: Vec<RuntimeValue>,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        callable.validate_for_owner(&RuntimeProgramOwner::Plan(Arc::clone(&self.plan)))?;
        if args.len() < callable.remaining_arity()? {
            return Ok(RuntimeValue::Callable(
                callable
                    .try_bind_prefix(args)
                    .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?,
            ));
        }
        let arguments = callable
            .materialize_arrow_arguments(args)
            .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
        let mut application = callable
            .prepare_group(arguments, None)
            .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
        loop {
            match application {
                RuntimeCallableApplication::Complete(value) => return Ok(value),
                RuntimeCallableApplication::Invoke(invocation) => {
                    let RuntimeCallableBodyReference::Plan(site) = invocation.body else {
                        return Err(RuntimeCallableValueError::ForeignProgram.into());
                    };
                    return self.evaluate_function_site(
                        site,
                        invocation.captures,
                        invocation.arguments,
                        backend,
                    );
                }
                RuntimeCallableApplication::AttachedDefault {
                    invocation,
                    pending,
                } => {
                    let RuntimeCallableBodyReference::Plan(site) = invocation.body else {
                        return Err(RuntimeCallableValueError::ForeignProgram.into());
                    };
                    let value = self.evaluate_function_site(
                        site,
                        invocation.captures,
                        invocation.arguments,
                        backend,
                    )?;
                    application = pending
                        .complete_default(value)
                        .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
                }
            }
        }
    }

    pub(in crate::engine) fn evaluate_function_site(
        &mut self,
        site: RuntimeFunctionSiteId,
        captures: Vec<RuntimeValue>,
        arguments: Vec<RuntimeValue>,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = Arc::clone(&self.plan);
        let declaration = plan.validate_function_site_inputs(site, &captures, &arguments)?;
        let type_instantiation = declaration.type_instantiation.clone();
        let RuntimeFunctionSiteBody::Expression(body) = declaration.body() else {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "structured.function".to_owned(),
                reason: "an executable runtime function requires function-call control transfer"
                    .to_owned(),
            });
        };
        let mut captures = captures.into_iter().map(Some).collect::<Vec<_>>();
        let mut arguments = arguments.into_iter().map(Some).collect::<Vec<_>>();
        let mut staged = Vec::new();
        for input in declaration.inputs() {
            let (values, position) = match input.source() {
                RuntimeFunctionInputSource::Capture { position } => (&mut captures, position),
                RuntimeFunctionInputSource::Parameter { position } => (&mut arguments, position),
            };
            let value = values
                .get_mut(position as usize)
                .and_then(Option::take)
                .ok_or(
                    crate::value::RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site },
                )?;
            let bindings = match_runtime_pattern_owned(
                &plan,
                input.pattern(),
                value,
                type_instantiation.as_deref(),
            )?
            .ok_or_else(|| {
                RuntimeEvalError::PatternMismatch(format!(
                    "function site {site} input {:?}",
                    input.source()
                ))
            })?;
            staged.extend(bindings);
        }
        self.fiber
            .env
            .push_function_scope(site, staged.len(), type_instantiation.clone());
        self.fiber.env.bind_all(staged);
        let value = self.evaluate_expr_with_backend(body, backend);
        let value = value.and_then(|value| {
            let matches = match &type_instantiation {
                Some(binding) => binding.value_matches(plan.as_ref(), declaration.result(), &value),
                None => plan.value_matches_type(declaration.result(), &value)?,
            };
            if matches {
                Ok(value)
            } else {
                Err(RuntimeEvalError::InvalidExpressionType(
                    declaration.result(),
                ))
            }
        });
        self.fiber.env.pop_scope();
        value
    }
}
