//! Pure evaluation of the same sealed callable-state transitions as Engine.

use std::sync::Arc;

use crate::plan::{RuntimeFunctionInputSource, RuntimeFunctionSiteBody};
use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallArgument, RuntimeCallableApplication, RuntimeCallableBodyReference,
    RuntimeCallableValue, RuntimeCallableValueError, RuntimeEvalError, RuntimeExpr, RuntimeValue,
    runtime_value_label,
};

use super::{PureEvaluator, match_runtime_pattern};

impl PureEvaluator<'_> {
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
        self.apply_runtime_function(&callable, &args)
    }

    pub(super) fn apply_runtime_function(
        &mut self,
        callable: &RuntimeCallableValue,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        callable.validate_for_owner(&RuntimeProgramOwner::Plan(Arc::clone(self.plan)))?;
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
                    )?;
                    application = callable.complete_group_default(&arguments, value)?;
                }
            }
        }
    }

    pub(super) fn evaluate_function_site(
        &mut self,
        site: RuntimeFunctionSiteId,
        captures: &[RuntimeValue],
        arguments: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let declaration = self
            .plan
            .validate_function_site_inputs(site, captures, arguments)?;
        let RuntimeFunctionSiteBody::Expression(body) = declaration.body() else {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "structured.function".to_owned(),
                reason: "an executable runtime function requires function-call control transfer"
                    .to_owned(),
            });
        };
        self.env
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
                let bindings = match_runtime_pattern(self.plan, input.pattern(), value)?
                    .ok_or_else(|| RuntimeEvalError::PatternMismatch(runtime_value_label(value)))?;
                self.env.set_ref(input.input_local(), value);
                self.env.bind_all(bindings);
            }
            self.evaluate_expr(body)
        })();
        self.env.pop_scope();
        value
    }
}
