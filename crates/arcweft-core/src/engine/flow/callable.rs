//! One same-fiber activation path for direct and value-based callable groups.

use std::sync::Arc;

use crate::engine::{
    Engine, FlowCursor, FunctionCallFrame, FunctionReturnContinuation, RuntimeCallBackend,
    RuntimeEvalError, RuntimeStepOutput, runtime_value_label,
};
use crate::pattern::RuntimePattern;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallableApplication, RuntimeCallableBodyReference, RuntimeCallablePendingGroup,
    RuntimeCallableValue, RuntimeCallableValueError, RuntimeValue,
};

impl Engine {
    pub(super) fn start_function_value_call(
        &mut self,
        callee: RuntimeValue,
        args: Vec<RuntimeValue>,
        result: RuntimePattern,
        resume: Option<FlowCursor>,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEvalError> {
        let RuntimeValue::Callable(callable) = callee else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &callee,
            )));
        };
        callable.validate_for_owner(&RuntimeProgramOwner::Plan(Arc::clone(&self.plan)))?;
        if args.len() < callable.remaining_arity()? {
            let value = RuntimeValue::Callable(
                callable
                    .try_bind_prefix(args)
                    .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?,
            );
            self.complete_function_call_result(&result, resume, value, output);
            return Ok(());
        }
        let arguments = callable
            .materialize_arrow_arguments(args)
            .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
        self.start_callable_group(callable, arguments, None, result, resume, output, backend)
    }

    pub(super) fn start_callable_group(
        &mut self,
        callable: RuntimeCallableValue,
        arguments: Vec<RuntimeValue>,
        attached: Option<RuntimeValue>,
        result: RuntimePattern,
        resume: Option<FlowCursor>,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEvalError> {
        callable.validate_for_owner(&RuntimeProgramOwner::Plan(Arc::clone(&self.plan)))?;
        let application = callable
            .prepare_group(arguments, attached)
            .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
        self.start_callable_application(application, result, resume, output, backend)
    }

    pub(super) fn finish_callable_group_default(
        &mut self,
        pending: RuntimeCallablePendingGroup,
        attached: RuntimeValue,
        result: RuntimePattern,
        resume: Option<FlowCursor>,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEvalError> {
        let application = pending
            .complete_default(attached)
            .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
        self.start_callable_application(application, result, resume, output, backend)
    }

    fn start_callable_application(
        &mut self,
        application: RuntimeCallableApplication,
        result: RuntimePattern,
        resume: Option<FlowCursor>,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEvalError> {
        match application {
            RuntimeCallableApplication::Complete(value) => {
                self.complete_function_call_result(&result, resume, value, output);
                Ok(())
            }
            RuntimeCallableApplication::Invoke(invocation) => {
                let RuntimeCallableBodyReference::Plan(site) = invocation.body else {
                    return Err(RuntimeCallableValueError::ForeignProgram.into());
                };
                let frame = FunctionCallFrame::new(
                    site,
                    resume,
                    FunctionReturnContinuation::Bind { result },
                );
                self.start_function_site_call(
                    invocation.captures,
                    invocation.arguments,
                    frame,
                    output,
                    backend,
                )
            }
            RuntimeCallableApplication::AttachedDefault {
                invocation,
                pending,
            } => {
                let RuntimeCallableBodyReference::Plan(site) = invocation.body else {
                    return Err(RuntimeCallableValueError::ForeignProgram.into());
                };
                let frame = FunctionCallFrame::new(
                    site,
                    resume,
                    FunctionReturnContinuation::CallableDefault { pending, result },
                );
                self.start_function_site_call(
                    invocation.captures,
                    invocation.arguments,
                    frame,
                    output,
                    backend,
                )
            }
        }
    }
}
