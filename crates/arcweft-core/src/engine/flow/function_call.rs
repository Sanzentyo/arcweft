//! Same-fiber invocation and return handling for structured function values.

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::engine::{
    Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowFiberStatus, FunctionCallFrame,
    FunctionReturnContinuation, RuntimeCallBackend, RuntimeEvalError, RuntimeStepOutput,
    runtime_value_label,
};
use crate::pattern::RuntimePattern;
use crate::plan::RuntimeFunctionSiteBody;
use crate::value::{RuntimeFunctionApplyError, RuntimeValue};

impl Engine {
    pub(in crate::engine) fn start_function_site_call(
        &mut self,
        captures: Vec<RuntimeValue>,
        args: Vec<RuntimeValue>,
        mut frame: FunctionCallFrame,
        next_op_index: Option<usize>,
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
            self.advance_if_needed(next_op_index);
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
            self.advance_if_needed(next_op_index);
            self.complete_function_call_return(frame, value, output, pure_backend);
            return Ok(());
        }
        self.stage_admitted_function_site_call(declaration, captures, args, frame, next_op_index)?;
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
        next_op_index: Option<usize>,
    ) -> Result<(), RuntimeEvalError> {
        let site = declaration.site();
        debug_assert_eq!(frame.site, site);
        let pending_ops = self.prepare_function_root_work(site)?;
        let staged = declaration.stage_owned_inputs(captures, args)?;
        self.advance_if_needed(next_op_index);
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
        self.fiber.frame.pending_ops = pending_ops;
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
                self.fiber.frame.invocation_result =
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
                self.fiber.frame.invocation_result =
                    Some((crate::engine::NativeInvocationRoot::Function(site), value));
                self.fiber.frame.status = FlowFiberStatus::Done(crate::engine::FlowExit::Done);
            }
            FunctionReturnContinuation::CallableDefault { pending, result } => {
                if let Err(error) = self.finish_callable_group_default(
                    pending,
                    value,
                    result,
                    None,
                    output,
                    pure_backend,
                ) {
                    self.fail_eval(error, output);
                }
            }
            FunctionReturnContinuation::Bind { result } => {
                self.complete_function_call_result(&result, None, value, output);
            }
        }
    }

    pub(super) fn complete_function_call_result(
        &mut self,
        result: &RuntimePattern,
        next_op_index: Option<usize>,
        value: RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        match self.try_bind_pattern_owned(result, value) {
            Ok(None) => {
                self.advance_if_needed(next_op_index);
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
    ) -> Result<Option<FunctionCallFrame>, RuntimeEvalError> {
        if !self
            .fiber
            .frame
            .control_stack
            .iter()
            .any(|entry| matches!(entry.kind, FlowControlStackEntryKind::FunctionCall(_)))
        {
            return Ok(None);
        }
        while let Some(entry) = self.fiber.frame.control_stack.last() {
            let owns_scope = match &entry.kind {
                FlowControlStackEntryKind::Scope { .. } => true,
                FlowControlStackEntryKind::FunctionCall(frame) => frame.function_scope,
                FlowControlStackEntryKind::FormatAttempt(frame) => frame.active.is_some(),
                _ => false,
            };
            if owns_scope {
                self.pop_runtime_scope()?;
            }
            let entry = self
                .fiber
                .frame
                .control_stack
                .pop()
                .expect("inspected function owner");
            match entry.kind {
                FlowControlStackEntryKind::Scope {
                    origin, cleanups, ..
                } => {
                    self.cancel_scheduled_scope_close(origin);
                    self.emit_scope_cleanups(cleanups, output, pure_backend);
                }
                FlowControlStackEntryKind::FunctionCall(mut frame) => {
                    self.fiber.frame.pending_ops = std::mem::take(&mut frame.caller_pending_ops);
                    return Ok(Some(frame));
                }
                _ => {}
            }
        }
        unreachable!("the selected function frame remains while unwinding its child scopes")
    }
    pub(in crate::engine) fn return_typed_value(
        &mut self,
        value: RuntimeValue,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Option<RuntimeValue> {
        let frame = match self.take_function_call_frame(output, pure_backend) {
            Ok(frame) => frame,
            Err(error) => {
                self.fail_eval(error, output);
                return None;
            }
        };
        if let Some(frame) = frame {
            self.complete_function_call_return(frame, value, output, pure_backend);
            return None;
        }
        if matches!(
            &self.fiber.launch,
            crate::engine::FlowFiberLaunch::Program(_)
        ) {
            self.complete_program_task_return(value, true, output, pure_backend);
            return None;
        }
        Some(value)
    }

    pub(super) fn complete_function_call_fallthrough(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let frame = match self.take_function_call_frame(output, pure_backend) {
            Ok(frame) => frame,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let Some(frame) = frame else {
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
