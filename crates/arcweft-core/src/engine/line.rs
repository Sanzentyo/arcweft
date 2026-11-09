use super::defer::{
    NativeDeferBoundary, NativeGotoOrigin, NativeLexicalExit, NativeLexicalExitAction,
};
use super::{
    Engine, FlowControlStackEntryKind, FlowEvent, FlowExit, FlowFiberStatus, LineEffectRequest,
    RuntimeStepInput, RuntimeStepOutput,
};
use crate::plan::{FlowOp, FlowRuntimeId, RuntimeFlowTransferTarget};
use crate::pure::RuntimeCallBackend;
use std::borrow::Borrow;

/// Exclusive preparation for the actual receiving root; no value is copied and
/// no presentation grant is issued until the transfer commits.
pub(super) struct NativePreparedFlowRoot {
    plan: std::sync::Arc<crate::plan::RuntimePlan>,
    target: FlowRuntimeId,
    site: crate::runtime_id::RuntimeFunctionSiteId,
    receiving: crate::value::RuntimeEnv,
    pending_ops: std::collections::VecDeque<super::NativePendingWork>,
    input_order: crate::value::RuntimeInputOrder,
    type_instantiation:
        Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    escaping: Vec<crate::runtime_id::RuntimePresentationHandleToken>,
}

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
            RuntimeFlowTransferTarget::Static(target) => {
                RuntimeFlowTransferTarget::Static(target.clone())
            }
            RuntimeFlowTransferTarget::Dynamic(expression) => {
                match self.evaluate_expr_with_backend(expression, backend) {
                    Ok(selector) => RuntimeFlowTransferTarget::Dynamic(selector),
                    Err(error) => {
                        self.fail_format_aware_eval(error, output, backend);
                        return;
                    }
                }
            }
        };
        let plan = std::sync::Arc::clone(&self.plan);
        let inputs = match self
            .fiber
            .frame
            .env
            .prepare_flow_inputs(&plan, &target, transfer)
        {
            Ok(inputs) => inputs,
            Err(error) => {
                self.fail_format_aware_eval(error, output, backend);
                return;
            }
        };
        if let Some(source) = source.as_ref()
            && self.fiber.owner.owns_flow_defers()
            && let Some(scope) = self.fiber.frame.defer_target(NativeDeferBoundary::Flow)
        {
            // Check the original body before moving any value into the
            // continuation. The final root is prepared after cleanup.
            if let Err(error) = self.prepare_function_root_work(inputs.declaration.site()) {
                self.fail_eval(error, output);
                return;
            }
            let source_instantiation = self.fiber.frame.env.function_instantiation_lease().cloned();
            let type_instantiation = inputs.declaration.type_instantiation.clone();
            let transfer = inputs
                .into_moves()
                .commit(&mut self.fiber.frame.env, target);
            self.fiber.frame.lexical_exit = Some(NativeLexicalExit {
                target: scope,
                action: NativeLexicalExitAction::Goto {
                    source: source.clone(),
                    transfer,
                    source_instantiation,
                    type_instantiation,
                },
            });
        } else {
            // Without executable cleanup, admit the whole receiving root
            // while the original slots still own the values, then transfer
            // them and retire the source contour in the producing operation.
            let prepared = match self.prepare_admitted_flow_root(
                &inputs.values,
                &inputs.declaration,
                inputs.order.clone(),
            ) {
                Ok(prepared) => prepared,
                Err(error) => {
                    self.fail_eval(error, output);
                    return;
                }
            };
            let transfer = inputs
                .into_moves()
                .commit(&mut self.fiber.frame.env, target);
            self.commit_flow_transfer_root(transfer, source.as_ref(), prepared, output, backend);
        }
    }

    pub(super) fn prepare_flow_transfer_root(
        &self,
        transfer: &crate::plan::RuntimeFlowTransfer<crate::value::RuntimeValue>,
        type_instantiation: Option<
            std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>,
        >,
    ) -> Result<NativePreparedFlowRoot, crate::value::RuntimeEvalError> {
        let (declaration, input_order) = self
            .plan
            .validate_flow_transfer_inputs(transfer)
            .map_err(|error| crate::value::RuntimeEvalError::FlowInvocation(Box::new(error)))?;
        if declaration.type_instantiation != type_instantiation {
            return Err(
                crate::value::RuntimeFunctionApplyError::InvalidEffectInstantiation {
                    site: declaration.site(),
                }
                .into(),
            );
        }
        self.prepare_admitted_flow_root(transfer, &declaration, input_order)
    }

    pub(super) fn prepare_admitted_flow_root<V: Borrow<crate::value::RuntimeValue>>(
        &self,
        transfer: &crate::plan::RuntimeFlowTransfer<V>,
        declaration: &crate::plan::RuntimeFunctionInputAdmission<'_>,
        input_order: crate::value::RuntimeInputOrder,
    ) -> Result<NativePreparedFlowRoot, crate::value::RuntimeEvalError> {
        let target = transfer.target.identity(|value| value.borrow().view())?;
        if self.fiber.frame.has_scope_defer_work() {
            return Err(crate::scope::RuntimeScopeExitError::DeferredWorkPending.into());
        }
        let site = declaration.site();
        let pending_ops = self.prepare_function_root_work(site)?;
        let escaping = Self::presentation_return_tokens(
            transfer
                .arguments
                .iter()
                .map(|argument| argument.value.borrow()),
        )?;
        let mut receiving = crate::value::RuntimeEnv::default();
        self.preflight_flow_presentation_root(&mut receiving, &escaping)?;
        Ok(NativePreparedFlowRoot {
            plan: std::sync::Arc::clone(&self.plan),
            target: target.clone(),
            site,
            receiving,
            pending_ops,
            input_order,
            type_instantiation: declaration.type_instantiation.clone(),
            escaping,
        })
    }

    pub(super) fn commit_flow_transfer_root(
        &mut self,
        transfer: crate::plan::RuntimeFlowTransfer<crate::value::RuntimeValue>,
        source: Option<&NativeGotoOrigin>,
        mut prepared: NativePreparedFlowRoot,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) {
        let (declaration, _) = prepared
            .plan
            .validate_flow_transfer_inputs(&transfer)
            .expect("exclusive root preparation retains the admitted input ABI and patterns");
        assert_eq!(declaration.site(), prepared.site);
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
                input_order: prepared.input_order,
            })
            .expect("the unchanged input patterns were admitted before any Move or cleanup");
        prepared
            .receiving
            .bind_root_function(prepared.site, prepared.type_instantiation, staged);
        self.commit_flow_presentation_root(&mut prepared.receiving, &prepared.escaping)
            .expect("exclusive root transition retains its preflighted actual source and destination grants");
        self.fiber.frame.await_observer = None;
        self.unwind_control_stack(output, backend)
            .expect("the admitted contour has no deferred work and its presentation grants have already transitioned");
        if let Some(FlowOp::Effect(effect @ LineEffectRequest::Goto(_))) =
            source.and_then(|source| match source {
                NativeGotoOrigin::Operation(source) => source.operation(&prepared.plan),
                NativeGotoOrigin::Choice { .. } => None,
            })
        {
            self.emit_line_effect(effect.clone(), output, backend);
        }
        output.flow_events.push(FlowEvent::Goto {
            target: prepared.target,
        });
        self.fiber.frame.env.install_flow_root(prepared.receiving);
        self.fiber.frame.enter_function_body(prepared.site);
        self.fiber.frame.pending_ops = prepared.pending_ops;
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
        if let FlowControlStackEntryKind::Scope { defers, .. } = &entry.kind
            && defers.has_work()
        {
            return Err(crate::scope::RuntimeScopeExitError::DeferredWorkPending.into());
        }
        if entry.kind.owns_runtime_scope() {
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
