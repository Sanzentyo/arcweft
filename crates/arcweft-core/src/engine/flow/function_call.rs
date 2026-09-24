//! Same-fiber invocation and return handling for structured function values.

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::engine::{
    Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowCursor, FlowFiberStatus,
    FunctionCallFrame, FunctionReturnContinuation, RuntimeCallBackend, RuntimeEvalError,
    RuntimeStepOutput, match_runtime_pattern, runtime_value_label,
};
use crate::pattern::RuntimePattern;
use crate::plan::{RuntimeFunctionInputSource, RuntimeFunctionSiteBody};
use crate::value::{RuntimeFunctionApplyError, RuntimeValue};

impl Engine {
    pub(super) fn start_function_site_call(
        &mut self,
        captures: Vec<RuntimeValue>,
        args: Vec<RuntimeValue>,
        mut frame: FunctionCallFrame,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEvalError> {
        let site = frame.site;
        let declaration = self
            .plan
            .validate_function_site_inputs(site, &captures, &args)?;
        let body = declaration.body().clone();
        let inputs = declaration.inputs().to_vec();
        if let RuntimeFunctionSiteBody::Expression(_) = &body {
            let value = self.evaluate_function_site(site, &captures, &args, pure_backend)?;
            frame.caller_pending_ops = std::mem::take(&mut self.fiber.pending_ops);
            self.complete_function_call_return(frame, value, output, pure_backend);
            return Ok(());
        }
        let RuntimeFunctionSiteBody::Executable(executable) = body else {
            unreachable!("function-site body match is exhaustive")
        };
        self.fiber.env.push_scope_with_capacity(inputs.len());
        let setup = (|| {
            for input in &inputs {
                let (values, position) = match input.source() {
                    RuntimeFunctionInputSource::Capture { position } => (&captures, position),
                    RuntimeFunctionInputSource::Parameter { position } => (&args, position),
                };
                let value = values
                    .get(usize::try_from(position).map_err(|_| {
                        RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site }
                    })?)
                    .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })?;
                self.fiber.env.set_ref(input.input_local(), value);
                let bindings = match_runtime_pattern(&self.plan, input.pattern(), value)?
                    .ok_or_else(|| RuntimeEvalError::PatternMismatch(runtime_value_label(value)))?;
                self.fiber.env.bind_all(bindings);
            }
            Ok::<(), RuntimeEvalError>(())
        })();
        if let Err(error) = setup {
            self.fiber.env.pop_scope();
            return Err(error);
        }
        frame.function_scope = true;
        frame.caller_pending_ops = std::mem::take(&mut self.fiber.pending_ops);
        self.fiber.control_stack.push(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FunctionCall(frame),
        });
        self.fiber
            .pending_ops
            .extend(executable.ops().iter().cloned());
        Ok(())
    }

    fn complete_function_call_return(
        &mut self,
        frame: FunctionCallFrame,
        value: RuntimeValue,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let plan_owner = Arc::clone(&self.plan);
        let Some(declaration) = plan_owner.function_sites().get(frame.site) else {
            self.fail_eval(
                RuntimeEvalError::FunctionApply(RuntimeFunctionApplyError::UnknownStructuredSite {
                    site: frame.site,
                }),
                output,
            );
            return;
        };
        match plan_owner.value_matches_type(declaration.result(), &value) {
            Ok(true) => {}
            Ok(false) => {
                self.fail_eval(
                    RuntimeEvalError::InvalidExpressionType(declaration.result()),
                    output,
                );
                return;
            }
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        }
        self.fiber.pending_ops = frame.caller_pending_ops;
        match frame.continuation {
            FunctionReturnContinuation::CallableDefault {
                callable,
                arguments,
                result,
            } => {
                if let Err(error) = self.finish_callable_group_default(
                    callable,
                    arguments,
                    value,
                    result,
                    frame.resume,
                    output,
                    pure_backend,
                ) {
                    self.fail_eval(error, output);
                }
            }
            FunctionReturnContinuation::Bind { result } => {
                self.complete_function_call_result(&result, frame.resume, value, output);
            }
        }
    }

    pub(super) fn complete_function_call_result(
        &mut self,
        result: &RuntimePattern,
        resume: Option<FlowCursor>,
        value: RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        match self.try_bind_pattern(result, &value) {
            Ok(true) => {
                self.fiber.cursor = resume;
                self.fiber.status = FlowFiberStatus::Running;
            }
            Ok(false) => self.fail_eval(
                RuntimeEvalError::PatternMismatch(runtime_value_label(&value)),
                output,
            ),
            Err(error) => self.fail_eval(error, output),
        }
    }

    fn take_function_call_frame(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Option<FunctionCallFrame> {
        if !self
            .fiber
            .control_stack
            .iter()
            .any(|entry| matches!(entry.kind, FlowControlStackEntryKind::FunctionCall(_)))
        {
            return None;
        }
        while let Some(entry) = self.fiber.control_stack.pop() {
            match entry.kind {
                FlowControlStackEntryKind::Scope { cleanups } => {
                    self.fiber.env.pop_scope();
                    self.emit_scope_cleanups(cleanups, output, pure_backend);
                }
                FlowControlStackEntryKind::FunctionCall(frame) => {
                    if frame.function_scope {
                        self.fiber.env.pop_scope();
                    }
                    return Some(frame);
                }
                FlowControlStackEntryKind::Loop { .. }
                | FlowControlStackEntryKind::While { .. }
                | FlowControlStackEntryKind::WhileLet { .. } => {}
            }
        }
        unreachable!("the selected function frame remains present while unwinding its child scopes")
    }

    pub(in crate::engine) fn return_function_call_value(
        &mut self,
        value: RuntimeValue,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(frame) = self.take_function_call_frame(output, pure_backend) else {
            return false;
        };
        self.complete_function_call_return(frame, value, output, pure_backend);
        true
    }

    pub(super) fn fail_function_call_fallthrough(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let Some(frame) = self.take_function_call_frame(output, pure_backend) else {
            unreachable!("fallthrough is dispatched only with an active function frame")
        };
        self.fiber.pending_ops.clear();
        self.fail_eval(
            RuntimeEvalError::FunctionFallthrough { site: frame.site },
            output,
        );
    }
}
