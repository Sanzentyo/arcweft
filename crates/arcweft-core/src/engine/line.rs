use super::defer::{
    NativeDeferBoundary, NativeGotoOrigin, NativeLexicalExit, NativeLexicalExitAction,
};
use super::{
    Engine, FlowControlStackEntryKind, FlowEvent, FlowExit, FlowFiberStatus, LineEffectRequest,
    RuntimeStepInput, RuntimeStepOutput,
};
use crate::plan::{FlowOp, FlowRuntimeId};
use crate::pure::RuntimeCallBackend;

impl Engine {
    pub(super) fn step_line_only(
        &mut self,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let _ = input;
        self.finish(output, pure_backend);
    }

    pub(super) fn apply_control_effects(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(control) = output.effects.line.iter().find_map(control_from_effect) else {
            return false;
        };
        match control {
            FlowControl::Goto(target) => self.goto_from_label(&target, None, output, pure_backend),
            FlowControl::Return(value) => self.return_value(value, output, pure_backend),
            FlowControl::Failed(message) => {
                self.fiber.frame.status = FlowFiberStatus::Failed(message)
            }
        }
        true
    }

    pub(super) fn goto_from_label(
        &mut self,
        label: &str,
        source: Option<NativeGotoOrigin>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        match self.plan.resolve_flow_target_value(label) {
            Ok(target) => self.goto(&target, source, output, pure_backend),
            Err(error) => {
                self.fiber.frame.status =
                    FlowFiberStatus::Failed(format!("invalid goto target {label}: {error}"));
            }
        }
    }

    pub(super) fn goto(
        &mut self,
        target: &FlowRuntimeId,
        source: Option<NativeGotoOrigin>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        self.start_flow_transfer(
            &crate::plan::RuntimeFlowTransfer::static_without_arguments(target.clone()),
            source,
            output,
            pure_backend,
        );
    }

    pub(super) fn start_flow_transfer(
        &mut self,
        transfer: &crate::plan::RuntimeFlowTransfer<crate::value::RuntimeExpr>,
        source: Option<NativeGotoOrigin>,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) {
        let target = match &transfer.target {
            crate::plan::RuntimeFlowTransferTarget::Static(target) => target.clone(),
            crate::plan::RuntimeFlowTransferTarget::Dynamic(expression) => {
                match self.evaluate_entity_target(expression, backend) {
                    Ok(target) => target,
                    Err(error) => {
                        self.fail_format_aware_eval(error, output, backend);
                        return;
                    }
                }
            }
        };
        let plan = std::sync::Arc::clone(&self.plan);
        let (site, _) = match plan.validate_flow_transfer_shape(&target, transfer) {
            Ok(admission) => admission,
            Err(error) => {
                self.fail_eval(
                    crate::value::RuntimeEvalError::FlowInvocation(Box::new(error)),
                    output,
                );
                return;
            }
        };
        // Original body construction is admitted while every source owner is
        // still in its caller slot.
        if let Err(error) = self.prepare_function_root_work(site) {
            self.fail_eval(error, output);
            return;
        }
        let source_instantiation = self.fiber.frame.env.function_instantiation_lease().cloned();
        let (transfer, type_instantiation) = match self
            .fiber
            .frame
            .env
            .transfer_flow_inputs(&plan, &target, transfer)
        {
            Ok(inputs) => inputs,
            Err(error) => {
                self.fail_format_aware_eval(error, output, backend);
                return;
            }
        };
        if source.is_some()
            && self.fiber.owner.owns_flow_defers()
            && let Some(scope) = self.fiber.frame.defer_target(NativeDeferBoundary::Flow)
        {
            self.fiber.frame.lexical_exit = Some(NativeLexicalExit {
                target: scope,
                action: NativeLexicalExitAction::Goto {
                    source: source.expect("the authored transfer retains its original operation"),
                    transfer,
                    source_instantiation,
                    type_instantiation,
                },
            });
        } else {
            self.activate_flow_transfer(
                transfer,
                source.as_ref(),
                type_instantiation,
                output,
                backend,
            );
        }
    }

    pub(super) fn activate_flow_transfer(
        &mut self,
        transfer: crate::plan::RuntimeFlowTransfer<crate::value::RuntimeValue>,
        source: Option<&NativeGotoOrigin>,
        type_instantiation: Option<
            std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>,
        >,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) {
        let crate::plan::RuntimeFlowTransferTarget::Static(target) = &transfer.target else {
            unreachable!("a prepared transition retains its resolved original Flow");
        };
        let target = target.clone();
        let plan = std::sync::Arc::clone(&self.plan);
        let (declaration, input_order) = plan
            .validate_flow_transfer_inputs(&target, &transfer)
            .expect("unchanged prepared inputs retain their admitted original target ABI");
        assert_eq!(
            declaration.type_instantiation, type_instantiation,
            "prepared transfer retains its original function instantiation"
        );
        let pending_ops = self
            .prepare_function_root_work(declaration.site())
            .expect("the immutable receiving body was admitted before moving any input");
        self.fiber.frame.await_observer = None;
        if let Err(error) = self.unwind_control_stack(output, backend) {
            self.fail_eval(error, output);
            return;
        }
        let mut arguments = vec![None; transfer.arguments.len()];
        for argument in transfer.arguments {
            arguments[argument.parameter.position() as usize] = Some(argument.value);
        }
        let arguments = arguments
            .into_iter()
            .map(|value| value.expect("the admitted input permutation covers every formal"))
            .collect();
        let staged = declaration
            .stage_owned_inputs(crate::value::RuntimeCallableInvocation {
                body: crate::value::RuntimeCallableBodyReference::Plan(declaration.site()),
                captures: Vec::new(),
                arguments,
                input_order,
            })
            .expect("the unchanged input patterns were admitted before any Move or cleanup");
        if let Some(FlowOp::Effect(effect @ LineEffectRequest::Goto(_))) =
            source.and_then(|source| match source {
                NativeGotoOrigin::Operation(source) => source.operation(&plan),
                NativeGotoOrigin::Choice { .. } => None,
            })
        {
            self.emit_line_effect(effect.clone(), output, backend);
        }
        output.flow_events.push(FlowEvent::Goto { target });
        self.fiber
            .frame
            .bind_root_function(declaration.site(), type_instantiation, staged);
        self.fiber.frame.pending_ops = pending_ops;
        self.fiber.frame.status = FlowFiberStatus::Running;
    }

    pub(super) fn return_value(
        &mut self,
        value: String,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let Some(value) = self.return_typed_value(
            crate::value::RuntimeValue::String(value),
            output,
            pure_backend,
        ) else {
            return;
        };
        let crate::value::RuntimeValue::String(value) = value else {
            unreachable!("the unhandled flow return retains its original string")
        };
        self.fiber.frame.pending_ops.clear();
        self.fiber.frame.await_observer = None;
        if let Err(error) = self.unwind_control_stack(output, pure_backend) {
            self.fail_eval(error, output);
            return;
        }
        output.flow_events.push(FlowEvent::Return {
            value: value.clone(),
        });
        self.fiber.frame.status = FlowFiberStatus::Done(FlowExit::Return(value));
    }

    pub(super) fn unwind_control_stack(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), crate::value::RuntimeEvalError> {
        while !self.fiber.frame.control_stack.is_empty() {
            self.unwind_control_entry(output, pure_backend)?;
        }
        self.drain_root_cleanups(output, pure_backend)?;
        self.fiber.frame.pending_ops.clear();
        self.fiber.frame.env.close_root_flow_registry();
        Ok(())
    }

    pub(super) fn unwind_control_entry(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), crate::value::RuntimeEvalError> {
        let Some(entry) = self.fiber.frame.control_stack.last() else {
            return Ok(());
        };
        let owns_scope = match &entry.kind {
            FlowControlStackEntryKind::Scope { defers, .. } => {
                if defers.has_work() {
                    return Err(crate::scope::RuntimeScopeExitError::DeferredWorkPending.into());
                }
                true
            }
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
            .expect("inspected control owner");
        match entry.kind {
            FlowControlStackEntryKind::Scope {
                origin, cleanups, ..
            } => {
                self.cancel_scheduled_scope_close(origin);
                self.emit_scope_cleanups(cleanups, output, pure_backend);
            }
            FlowControlStackEntryKind::FunctionCall(frame) => {
                self.fiber.frame.pending_ops = frame.caller_pending_ops;
            }
            FlowControlStackEntryKind::FormatAttempt(frame) => {
                if let Some(active) = frame.active {
                    self.fiber.frame.pending_ops = active.caller_pending_ops;
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn finish(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if self.defer_function_fallthrough(output) {
            return;
        }
        if matches!(&self.fiber.launch, super::FlowFiberLaunch::Program(_))
            && self.fiber.frame.invocation_result.is_none()
        {
            self.complete_program_task_return(
                crate::value::RuntimeValue::Unit,
                false,
                output,
                pure_backend,
            );
            return;
        }
        self.fiber.frame.pending_ops.clear();
        if let Err(error) = self.unwind_control_stack(output, pure_backend) {
            self.fail_eval(error, output);
            return;
        }
        output.flow_events.push(FlowEvent::Done);
        self.fiber.frame.status = FlowFiberStatus::Done(FlowExit::Done);
    }
}

enum FlowControl {
    Goto(String),
    Return(String),
    Failed(String),
}

fn control_from_effect(effect: &LineEffectRequest) -> Option<FlowControl> {
    match effect {
        LineEffectRequest::Goto(target) => Some(FlowControl::Goto(target.clone())),
        LineEffectRequest::Return(value) => Some(FlowControl::Return(value.clone())),
        LineEffectRequest::Panic(message)
        | LineEffectRequest::Fail(message)
        | LineEffectRequest::Bail(message) => Some(FlowControl::Failed(message.clone())),
        _ => None,
    }
}
