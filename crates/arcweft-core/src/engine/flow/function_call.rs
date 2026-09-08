//! Same-fiber invocation and return handling for structured function values.

use std::sync::Arc;

use crate::engine::{
    Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowCursor, FlowFiberStatus,
    FunctionCallFrame, FunctionReturnContinuation, RuntimeCallBackend, RuntimeEvalError,
    RuntimeStepOutput, match_runtime_pattern, runtime_value_label,
};
use crate::pattern::RuntimePattern;
use crate::plan::{
    RuntimeFunctionInputSource, RuntimeFunctionSiteBody, RuntimeProjectCallAttachedPresence,
};
use crate::value::{RuntimeFunctionApplyError, RuntimeFunctionValue, RuntimeValue};

impl Engine {
    pub(super) fn start_function_value_call(
        &mut self,
        callee: RuntimeValue,
        args: Vec<RuntimeValue>,
        result: RuntimePattern,
        resume: Option<FlowCursor>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEvalError> {
        let RuntimeValue::Function(function) = callee else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &callee,
            )));
        };
        let Some(closure) = function.as_structured() else {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "awbc.function".to_owned(),
                reason: "structured runtime cannot enter an AWBC function body".to_owned(),
            });
        };
        if !Arc::ptr_eq(&self.plan, closure.plan()) {
            return Err(RuntimeEvalError::ForeignStructuredFunction {
                site: closure.site(),
            });
        }
        let remaining = function.remaining_arity()?;
        if args.len() < remaining {
            let value = RuntimeValue::Function(function.try_bind_prefix(&args)?);
            self.complete_function_call_result(&result, resume, value, output);
            return Ok(());
        }
        let (call_args, remaining_args) = args.split_at(remaining);
        function.validate_bind_prefix(call_args)?;
        let mut parameter_values = closure.bound_args().to_vec();
        parameter_values.extend_from_slice(call_args);
        let frame = FunctionCallFrame::new(
            closure.site(),
            resume,
            FunctionReturnContinuation::Bind {
                result,
                remaining_args: remaining_args.to_vec(),
            },
        );
        self.start_function_site_call(
            closure.capture_values().to_vec(),
            parameter_values,
            frame,
            output,
            pure_backend,
        )
    }

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
            .function_sites()
            .get(site)
            .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
        let body = declaration.body().clone();
        let inputs = declaration.inputs().to_vec();
        let function =
            RuntimeFunctionValue::capture_site(Arc::clone(&self.plan), site, captures.clone())?;
        let expected = function.remaining_arity()?;
        if args.len() != expected {
            return Err(RuntimeEvalError::FunctionArgumentCount {
                expected,
                found: args.len(),
            });
        }
        function.validate_bind_prefix(&args)?;
        if let RuntimeFunctionSiteBody::Expression(_) = &body {
            let value = self.apply_runtime_function(&function, &args, pure_backend)?;
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
            FunctionReturnContinuation::ProjectDefault {
                site,
                prefix_values,
                mut logical_values,
            } => {
                let Some(row) = plan_owner.project_call_sites().get(site) else {
                    self.fiber.status =
                        FlowFiberStatus::Failed(format!("missing project-call site {site}"));
                    return;
                };
                let Some(attached) = row.plan().attached() else {
                    self.fail_eval(
                        RuntimeEvalError::InvalidExpressionType(row.result().ty()),
                        output,
                    );
                    return;
                };
                if !matches!(attached.presence(), RuntimeProjectCallAttachedPresence::DefaultedOmitted(default) if default.site() == frame.site)
                {
                    self.fail_eval(
                        RuntimeEvalError::InvalidExpressionType(attached.binding_ty()),
                        output,
                    );
                    return;
                }
                match plan_owner.value_matches_type(attached.binding_ty(), &value) {
                    Ok(true) => {}
                    Ok(false) => {
                        self.fail_eval(
                            RuntimeEvalError::InvalidExpressionType(attached.binding_ty()),
                            output,
                        );
                        return;
                    }
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                }
                logical_values.push(value);
                self.finish_project_call_terminal(
                    site,
                    prefix_values,
                    logical_values,
                    frame.resume,
                    output,
                    pure_backend,
                );
            }
            FunctionReturnContinuation::Bind {
                result,
                remaining_args,
            } => {
                if remaining_args.is_empty() {
                    self.complete_function_call_result(&result, frame.resume, value, output);
                } else if let Err(error) = self.start_function_value_call(
                    value,
                    remaining_args,
                    result,
                    frame.resume,
                    output,
                    pure_backend,
                ) {
                    self.fail_eval(error, output);
                }
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
