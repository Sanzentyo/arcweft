//! Same-fiber invocation and return handling for structured function values.

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::engine::{
    Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowFiberStatus, FunctionCallFrame,
    FunctionReturnContinuation, NativeCodeCursor, RuntimeCallBackend, RuntimeEvalError,
    RuntimeStepOutput, runtime_value_label,
};
use crate::pattern::RuntimePattern;
use crate::plan::RuntimeFunctionSiteBody;
use crate::plan::code::RuntimeCodeOwner;
use crate::value::{RuntimeFunctionApplyError, RuntimeValue};

impl Engine {
    pub(in crate::engine) fn start_function_site_call(
        &mut self,
        captures: Vec<RuntimeValue>,
        args: Vec<RuntimeValue>,
        mut frame: FunctionCallFrame,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEvalError> {
        let site = frame.site;
        let plan_owner = Arc::clone(&self.plan);
        let declaration = plan_owner.validate_function_site_inputs(site, &captures, &args)?;
        let executable = matches!(declaration.body(), RuntimeFunctionSiteBody::Executable(_));
        if executable
            && declaration.is_eager_pure_candidate()
            && let Ok(function) = crate::pure::RuntimePureFunctionRef::resolve(&plan_owner, site)
            && let Some(value) = self.evaluate_budgeted_scalar_function_site(
                function,
                &captures,
                &args,
                pure_backend,
            )?
        {
            // VM runtime backends decline executable scalar calls so the
            // original budgeted frame remains active. Only an admitted AOT/JIT
            // capability can complete this physical fast path.
            frame.type_instantiation = declaration.type_instantiation;
            frame.caller_pending_ops = std::mem::take(&mut self.fiber.frame.pending_ops);
            self.complete_function_call_return(frame, value, output, pure_backend);
            return Ok(());
        }
        if let RuntimeFunctionSiteBody::Expression(_) = declaration.body()
            && !matches!(
                frame.continuation,
                FunctionReturnContinuation::Program { .. }
                    | FunctionReturnContinuation::Function { .. }
            )
        {
            let value = self.evaluate_function_site(site, captures, args, pure_backend)?;
            frame.type_instantiation = declaration.type_instantiation;
            frame.caller_pending_ops = std::mem::take(&mut self.fiber.frame.pending_ops);
            self.complete_function_call_return(frame, value, output, pure_backend);
            return Ok(());
        }
        self.stage_admitted_function_site_call(declaration, captures, args, frame)?;
        if executable
            && let Ok(function) = crate::pure::RuntimePureFunctionRef::resolve(&plan_owner, site)
            && function.scalar_eval_supported
            && function
                .function_site()
                .is_some_and(|site| site.is_eager_pure_candidate())
        {
            pure_backend.record_interpreted_function_call(function);
        }
        Ok(())
    }

    /// Installs the already admitted site and owned inputs without evaluating
    /// its body. Activation and budgeted calls share this frame boundary;
    /// only the owning execution step supplies a backend for actual work.
    pub(in crate::engine) fn stage_admitted_function_site_call(
        &mut self,
        declaration: crate::plan::RuntimeFunctionInputAdmission<'_>,
        captures: Vec<RuntimeValue>,
        args: Vec<RuntimeValue>,
        mut frame: FunctionCallFrame,
    ) -> Result<(), RuntimeEvalError> {
        let site = declaration.site();
        debug_assert_eq!(frame.site, site);
        let cursor = NativeCodeCursor::root(&self.plan, RuntimeCodeOwner::Function(site))
            .expect("an admitted function retains its original code row");
        let staged = declaration.stage_owned_inputs(captures, args)?;
        frame.type_instantiation = declaration.type_instantiation;
        self.fiber.frame.env.push_function_scope(
            site,
            staged.len(),
            frame.type_instantiation.clone(),
        );
        self.fiber.frame.env.bind_all(staged);
        frame.function_scope = true;
        frame.caller_pending_ops = std::mem::take(&mut self.fiber.frame.pending_ops);
        self.fiber.frame.control_stack.push(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FunctionCall(frame),
        });
        self.fiber.frame.cursor = Some(cursor);
        Ok(())
    }

    /// Fetches one root from the original function row. Nested control work is
    /// drained first; each suspended caller retains its own root position.
    pub(super) fn step_function_site_body(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
        drop_policy: &mut Option<crate::effect::RuntimeDropPolicy>,
    ) -> bool {
        let Some(frame) = self.fiber.frame.active_function() else {
            return false;
        };
        let site = frame.site;
        let plan = Arc::clone(&self.plan);
        let Some(cursor) = self.fiber.frame.cursor.as_ref() else {
            self.fail_eval("active function lost its original code position", output);
            return true;
        };
        if cursor.owner() != RuntimeCodeOwner::Function(site) {
            self.fail_eval(
                "active function references another original code owner",
                output,
            );
            return true;
        }
        if let Some(operations) = cursor.operations(&plan) {
            if let Some(operation) = operations.first() {
                let next = cursor.operation_index().expect("operation code position") + 1;
                self.execute_flow_operation(
                    std::borrow::Cow::Borrowed(operation),
                    Some(next),
                    output,
                    pure_backend,
                    drop_policy,
                );
            } else {
                self.complete_function_call_fallthrough(output, pure_backend);
            }
        } else if let Some(expression) = cursor.expression(&plan) {
            match self.evaluate_expr_with_backend(expression, pure_backend) {
                Ok(value) => {
                    let returned = self.return_function_call_value(value, output, pure_backend);
                    debug_assert!(returned, "the active function retains its return frame");
                }
                Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
            }
        } else {
            self.fail_eval(
                RuntimeEvalError::FunctionApply(RuntimeFunctionApplyError::UnknownStructuredSite {
                    site,
                }),
                output,
            );
        }
        true
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
        let matches = match &frame.type_instantiation {
            Some(binding) => {
                Ok(binding.value_matches(plan_owner.as_ref(), declaration.result(), &value))
            }
            None => plan_owner.value_matches_type(declaration.result(), &value),
        };
        match matches {
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
        self.fiber.frame.pending_ops = frame.caller_pending_ops;
        match frame.continuation {
            FunctionReturnContinuation::RootInitializer { entry } => {
                if let Err(error) = self.complete_root_initializer(&entry, value, output) {
                    self.fail_eval(error, output);
                }
            }
            FunctionReturnContinuation::RootReducer { entry } => {
                if self.fiber.owner
                    != (super::super::FlowFiberOwner::RootReducer {
                        entry: entry.clone(),
                    })
                {
                    self.fail_eval(
                        "native reducer returned from a different fiber owner",
                        output,
                    );
                    return;
                }
                match self.finish_root_reduction(&entry, Ok(value), output) {
                    Ok(()) => {
                        self.fiber.frame.cursor = None;
                        self.fiber.frame.status =
                            FlowFiberStatus::Done(crate::engine::FlowExit::Done);
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FunctionReturnContinuation::Program { program } => {
                if !plan_owner
                    .resolve_pure_program(program)
                    .is_ok_and(|binding| binding.site() == frame.site)
                {
                    self.fail_eval(
                        RuntimeEvalError::UnsupportedPure {
                            name: program.to_string(),
                            reason: "program return does not match its admitted function site"
                                .to_owned(),
                        },
                        output,
                    );
                    return;
                }
                self.fiber.frame.cursor = None;
                self.invocation_result =
                    Some((crate::engine::NativeInvocationRoot::Program(program), value));
                self.fiber.frame.status = FlowFiberStatus::Done(crate::engine::FlowExit::Done);
            }
            FunctionReturnContinuation::Function { site } => {
                if site != frame.site
                    || crate::pure::RuntimePureFunctionRef::resolve(&plan_owner, site).is_err()
                {
                    self.fail_eval(
                        RuntimeEvalError::UnsupportedPure {
                            name: "structured.function".to_owned(),
                            reason: "function return does not match its admitted invocation root"
                                .to_owned(),
                        },
                        output,
                    );
                    return;
                }
                self.fiber.frame.cursor = None;
                self.invocation_result =
                    Some((crate::engine::NativeInvocationRoot::Function(site), value));
                self.fiber.frame.status = FlowFiberStatus::Done(crate::engine::FlowExit::Done);
            }
            FunctionReturnContinuation::CallableDefault { pending, result } => {
                if let Err(error) = self.finish_callable_group_default(
                    pending,
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
        resume: Option<NativeCodeCursor>,
        value: RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        match self.try_bind_pattern_owned(result, value) {
            Ok(None) => {
                self.fiber.frame.cursor = resume;
                self.fiber.frame.status = FlowFiberStatus::Running;
            }
            Ok(Some(value)) => self.fail_eval(
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
            .frame
            .control_stack
            .iter()
            .any(|entry| matches!(entry.kind, FlowControlStackEntryKind::FunctionCall(_)))
        {
            return None;
        }
        while let Some(entry) = self.fiber.frame.control_stack.pop() {
            match entry.kind {
                FlowControlStackEntryKind::Scope {
                    origin, cleanups, ..
                } => {
                    self.cancel_scheduled_scope_close(origin);
                    self.fiber.frame.env.pop_scope();
                    self.emit_scope_cleanups(cleanups, output, pure_backend);
                }
                FlowControlStackEntryKind::FunctionCall(frame) => {
                    if frame.function_scope {
                        self.fiber.frame.env.pop_scope();
                    }
                    return Some(frame);
                }
                FlowControlStackEntryKind::Loop { .. }
                | FlowControlStackEntryKind::While { .. }
                | FlowControlStackEntryKind::WhileLet { .. } => {}
                FlowControlStackEntryKind::FormatAttempt(frame) => {
                    if frame.active.is_some() {
                        self.fiber.frame.env.pop_scope();
                    }
                }
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

    pub(super) fn complete_function_call_fallthrough(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let Some(frame) = self.take_function_call_frame(output, pure_backend) else {
            unreachable!("fallthrough is dispatched only with an active function frame")
        };
        let completes_unit = self
            .plan
            .function_sites()
            .get(frame.site)
            .is_some_and(|site| {
                site.role().allows_implicit_unit_completion()
                    && matches!(
                        self.plan.checked_type(site.result()),
                        Ok(Some(crate::pattern::RuntimeCheckedType::Unit))
                    )
            });
        if completes_unit {
            self.complete_function_call_return(frame, RuntimeValue::Unit, output, pure_backend);
            return;
        }
        self.fiber.frame.pending_ops.clear();
        self.fail_eval(
            RuntimeEvalError::FunctionFallthrough { site: frame.site },
            output,
        );
    }
}
