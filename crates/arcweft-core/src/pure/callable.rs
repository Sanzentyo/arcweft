//! Pure evaluation of the same sealed callable-state transitions as Engine.

use std::sync::Arc;

use crate::pattern::match_runtime_pattern_owned;
use crate::plan::{RuntimeFunctionInputSource, RuntimeFunctionSiteBody};
use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallArgument, RuntimeCallableApplication, RuntimeCallableBodyReference,
    RuntimeCallableValue, RuntimeCallableValueError, RuntimeEvalError, RuntimeExpr, RuntimeValue,
    runtime_value_label,
};

use super::PureEvaluator;

impl PureEvaluator<'_> {
    pub(super) fn evaluate_specialize_callable_expr(
        &mut self,
        value: &RuntimeExpr,
        specialization: crate::runtime_id::RuntimeCallableSpecializationId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(value)?;
        let RuntimeValue::Callable(callable) = value else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &value,
            )));
        };
        callable
            .specialize(
                &RuntimeProgramOwner::Plan(Arc::clone(self.plan)),
                specialization,
            )
            .map(RuntimeValue::Callable)
            .map_err(Into::into)
    }

    pub(super) fn evaluate_callable_expr(
        &mut self,
        state: RuntimeCallableStateId,
        captures: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let captures = captures
            .iter()
            .map(|capture| self.evaluate_expr(capture))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::Callable(RuntimeCallableValue::try_new(
            RuntimeProgramOwner::Plan(Arc::clone(self.plan)),
            state,
            captures,
        )?))
    }

    pub(super) fn evaluate_apply_expr(
        &mut self,
        callee: &RuntimeExpr,
        args: &[RuntimeCallArgument],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let callee = self.evaluate_expr(callee)?;
        let args = self.evaluate_call_args(args)?;
        let RuntimeValue::Callable(callable) = callee else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &callee,
            )));
        };
        self.apply_runtime_function(callable, args)
    }

    pub(super) fn apply_runtime_function(
        &mut self,
        callable: RuntimeCallableValue,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        callable.validate_for_owner(&RuntimeProgramOwner::Plan(Arc::clone(self.plan)))?;
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
                    )?;
                    application = pending
                        .complete_default(value)
                        .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
                }
            }
        }
    }

    pub(super) fn evaluate_function_site(
        &mut self,
        site: RuntimeFunctionSiteId,
        captures: Vec<RuntimeValue>,
        arguments: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let declaration = self
            .plan
            .validate_function_site_inputs(site, &captures, &arguments)?;
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
                RuntimeFunctionInputSource::Parameter { position, .. } => {
                    (&mut arguments, position)
                }
            };
            let value = values
                .get_mut(position as usize)
                .and_then(Option::take)
                .ok_or(
                    crate::value::RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site },
                )?;
            let bindings = match_runtime_pattern_owned(
                self.plan,
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
        self.env
            .push_function_scope(site, staged.len(), type_instantiation.clone());
        self.env.bind_all(staged);
        let value = self.evaluate_expr(body);
        let value = value.and_then(|value| {
            let matches = match &type_instantiation {
                Some(binding) => {
                    binding.value_matches(self.plan.as_ref(), declaration.result(), &value)
                }
                None => self.plan.value_matches_type(declaration.result(), &value)?,
            };
            if matches {
                Ok(value)
            } else {
                Err(RuntimeEvalError::InvalidExpressionType(
                    declaration.result(),
                ))
            }
        });
        self.env.pop_scope();
        value
    }
}
