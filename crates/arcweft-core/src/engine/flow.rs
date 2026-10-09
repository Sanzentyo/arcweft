mod callable;
mod format_attempt;
mod function_call;
pub(super) mod match_guard;

use super::dialogue::{DialogueActivationFrame, DialogueLineTaskState};
use super::{
    ChoiceState, Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowCursor, FlowEvent,
    FlowFiberStatus, FlowOp, FlowScopeCleanup, HostCallState, RuntimeDiagnostic, RuntimeEvalError,
    RuntimeExpr, RuntimeIterator, RuntimePattern, RuntimeStepOutput, RuntimeValue,
    runtime_value_label,
};
use crate::effect::LineEffectRequest;
use crate::pattern::{RuntimeBuiltinVariantCaseIdentity, pattern_binding_capacity};
use crate::plan::{
    RuntimeIteratorEvidence, RuntimeIteratorWitnessExecutable,
    RuntimeProjectCallOrdinaryMaterialization, RuntimeReceiverMode,
};
use crate::pure::RuntimeCallBackend;
use crate::scope::RuntimeScopeIdentity;
use crate::step::{RuntimeHostCallId, RuntimeHostCallRequest};
use crate::task::{HostTaskRequest, NamedHostArg, RuntimeHostArgumentTemplate};
use crate::time::LogicalDuration;
use crate::value::{RuntimeEnv, RuntimeLocalBinding};
use std::sync::Arc;

impl Engine {
    // Keep the opcode dispatcher contiguous while the Phase 1 runtime surface is
    // still changing; extracting each arm now would obscure grammar coverage.
    #[allow(clippy::too_many_lines)]
    pub(super) fn step_flow(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
        drop_policy: &mut Option<crate::effect::RuntimeDropPolicy>,
    ) {
        if self.fiber.pending_ops.is_empty() && self.has_active_project_call() {
            self.fail_function_call_fallthrough(output, pure_backend);
            return;
        }
        let (op, next_op_index) = if let Some(op) = self.fiber.pending_ops.pop_front() {
            (op, None)
        } else {
            let Some(cursor) = self.fiber.cursor.as_ref() else {
                return;
            };
            let Some(op) = self
                .flow_at_cursor(cursor)
                .and_then(|flow| flow.body().ops().get(cursor.op_index))
                .cloned()
            else {
                self.finish(output, pure_backend);
                return;
            };
            (op, Some(cursor.op_index + 1))
        };
        match op {
            FlowOp::Bind(bindings) => {
                self.fiber.env.bind_all(bindings);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::Let { pattern, expr } => {
                self.evaluate_let_with_backend(&pattern, &expr, output, pure_backend);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::FormatOperandAttempt {
                attempt,
                parameter,
                body,
                value,
            } => {
                if let Err(error) = self.start_format_operand_attempt(
                    attempt,
                    parameter,
                    body,
                    value,
                    next_op_index,
                ) {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowOp::CompleteFormatOperand {
                attempt,
                parameter,
                value,
            } => {
                if let Err(error) =
                    self.complete_format_operand_attempt(attempt, parameter, &value, pure_backend)
                {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowOp::LetElse {
                pattern,
                expr,
                else_ops,
            } => {
                match self
                    .evaluate_expr_with_backend(&expr, pure_backend)
                    .and_then(|value| self.try_bind_pattern_owned(&pattern, value))
                {
                    Ok(None) => self.advance_if_needed(next_op_index),
                    Ok(Some(value)) => {
                        self.advance_if_needed(next_op_index);
                        self.push_ops(else_ops);
                        output.diagnostics.push(RuntimeDiagnostic::new(format!(
                            "let-else pattern did not match {}",
                            runtime_value_label(&value)
                        )));
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::Assign { place, value } => {
                match self.evaluate_expr_with_backend(&value, pure_backend) {
                    Ok(value) => match self.fiber.env.assign_place(&place, value) {
                        Ok(_) => self.advance_if_needed(next_op_index),
                        Err(error) => self.fail_eval(error.into_parts().0, output),
                    },
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::LineOperation { .. } | FlowOp::CommitDialogueResult { .. } => {
                self.fiber.status = FlowFiberStatus::Failed(
                    "line operation escaped its dialogue activation authority".to_owned(),
                );
            }
            FlowOp::SelectDialogueResult { value } => {
                if !matches!(
                    &self.fiber.owner,
                    super::FlowFiberOwner::LineTask(owner)
                        if matches!(
                            owner.tag.work(),
                            crate::line_task::LineTaskWork::Cancellation(_)
                                | crate::line_task::LineTaskWork::Node(_)
                        )
                ) || self.fiber.selected_dialogue_result.is_some()
                {
                    self.fail_eval(
                        crate::line_task::LineRuntimeError::InvalidActivationOperation,
                        output,
                    );
                    return;
                }
                match self.evaluate_expr_with_backend(&value, pure_backend) {
                    Ok(selected) => {
                        self.fiber.selected_dialogue_result = Some(selected);
                        *drop_policy = Some(crate::effect::RuntimeDropPolicy::Default);
                        self.fiber.pending_ops.clear();
                        self.fiber.cursor = None;
                        self.unwind_control_stack(output, pure_backend);
                        self.fiber.status = FlowFiberStatus::Done(super::FlowExit::Done);
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::Dialogue {
                target,
                content,
                result,
            } => {
                let target_value = match self.evaluate_expr_with_backend(&target, pure_backend) {
                    Ok(value) => value,
                    Err(error) => {
                        self.fail_format_aware_eval(error, output, pure_backend);
                        return;
                    }
                };
                if let Err(error) = self.plan.accepts_value(
                    target.ty(),
                    &target_value,
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                ) {
                    self.fail_eval(
                        crate::value::RuntimeEvalError::CharacterDialogueConstruction(
                            error.to_string(),
                        ),
                        output,
                    );
                    return;
                }
                let crate::value::RuntimeValue::Opaque(target_value) = target_value else {
                    self.fail_eval(
                        crate::value::RuntimeEvalError::CharacterDialogueConstruction(
                            "dialogue target is not an opaque CharacterDialogue value".to_owned(),
                        ),
                        output,
                    );
                    return;
                };
                let Some(content_plan) = self.plan.dialogue_content().get(content).cloned() else {
                    self.fiber.status =
                        FlowFiberStatus::Failed(format!("missing dialogue content plan {content}"));
                    return;
                };
                let mut values = Vec::with_capacity(content_plan.values().len());
                for site in content_plan.values() {
                    match self.evaluate_dialogue_site(site, pure_backend) {
                        Ok(value) => values.push(crate::plan::RuntimeDialogueValueBinding {
                            slot: site.slot(),
                            role: site.role(),
                            value,
                        }),
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    }
                }
                let mut effect_callbacks = Vec::with_capacity(content_plan.effect_sites().len());
                for site in content_plan.effect_sites() {
                    let callback = match self.evaluate_callable_expr(
                        site.state(),
                        site.captures(),
                        pure_backend,
                    ) {
                        Ok(RuntimeValue::Callable(callback)) => callback,
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                        Ok(_) => unreachable!(
                            "structured function-site construction returned a non-function"
                        ),
                    };
                    let remaining = match callback.remaining_arity() {
                        Ok(remaining) => remaining,
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    };
                    if remaining != 0 || !callback.is_structured_executable_callback() {
                        self.fiber.status = FlowFiberStatus::Failed(
                            "dialogue effect callback is not an executable zero-argument function"
                                .to_owned(),
                        );
                        return;
                    }
                    effect_callbacks.push(crate::value::RuntimeDialogueContentEffectBinding::new(
                        site.site(),
                        callback,
                    ));
                }
                let line = content_plan.line().clone();
                let Some(task_group) = content_plan.line_task_group() else {
                    self.fiber.status = FlowFiberStatus::Failed(
                        crate::line_task::LineRuntimeError::MissingTaskGroup.to_string(),
                    );
                    return;
                };
                let Some(group) = self
                    .plan
                    .line_task_groups()
                    .get(task_group.index())
                    .cloned()
                else {
                    self.fiber.status = FlowFiberStatus::Failed(
                        crate::line_task::LineRuntimeError::UnknownTaskGroup.to_string(),
                    );
                    return;
                };
                if group.result_type() != result.ty() {
                    self.fiber.status = FlowFiberStatus::Failed(
                        crate::line_task::LineRuntimeError::DialogueResultTypeMismatch.to_string(),
                    );
                    return;
                }
                let captures = match self.capture_line_task_locals(&group) {
                    Ok(captures) => captures,
                    Err(error) => {
                        self.fiber.status = FlowFiberStatus::Failed(error.to_string());
                        return;
                    }
                };
                let activation = match self.allocate_dialogue_activation(content) {
                    Ok(activation) => activation,
                    Err(message) => {
                        self.fiber.status = FlowFiberStatus::Failed(message.to_string());
                        return;
                    }
                };
                self.advance_if_needed(next_op_index);
                let mut locals = RuntimeEnv::default();
                let capture_locals = captures
                    .iter()
                    .map(|capture| capture.local)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                locals.bind_all(captures.into_vec());
                let dialogue = DialogueActivationFrame {
                    line,
                    content,
                    target: Some(target_value),
                    task_group,
                    resume: self.fiber.cursor,
                    captures: capture_locals,
                    task_inputs: Box::new([]),
                    locals,
                    line_task: DialogueLineTaskState::NotStarted,
                    elapsed: LogicalDuration::default(),
                    phase: super::DialogueRuntimePhase::Activating,
                    result_target: result,
                    voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
                    values: values.into_boxed_slice(),
                    effect_callbacks: effect_callbacks.into_boxed_slice(),
                    activation_pc: 0,
                    exiting_for_result: false,
                    scopes: Vec::new(),
                    pending_line_operation: None,
                    pending_host_call: None,
                    failure: None,
                };
                if let Err(error) = self
                    .dialogue_activations
                    .begin(activation.clone(), dialogue)
                {
                    self.fiber.status = FlowFiberStatus::Failed(error.to_string());
                    return;
                }
                self.fiber.status = FlowFiberStatus::Dialogue(super::DialogueExecutionStatus::new(
                    activation,
                    super::DialogueRuntimePhase::Activating,
                ));
            }
            FlowOp::Choice { id, options } => {
                output.flow_events.push(FlowEvent::ChoicePresented {
                    id: id.clone(),
                    options: options.clone(),
                });
                self.fiber.status = FlowFiberStatus::Choice(ChoiceState {
                    id,
                    options,
                    resume: self.resume_cursor(next_op_index),
                });
            }
            FlowOp::Await {
                binding,
                target,
                observers,
            } => {
                self.start_need_await(
                    binding,
                    target,
                    observers,
                    self.resume_cursor(next_op_index),
                    output,
                    pure_backend,
                );
            }
            FlowOp::StartNeedProducer { binding, target } => {
                self.start_need_producer(binding, target, next_op_index, output, pure_backend);
            }
            FlowOp::AwaitMany {
                binding,
                target,
                pending,
            } => {
                self.emit_line_effects(pending, output, pure_backend);
                self.start_await_many_state(
                    binding,
                    target,
                    self.resume_cursor(next_op_index),
                    output,
                    pure_backend,
                );
            }
            FlowOp::HostCall { binding, target } => {
                let arguments = match self.evaluate_host_call_arguments(&target.args, pure_backend)
                {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        self.fiber.status = FlowFiberStatus::Failed(error.clone());
                        output.diagnostics.push(RuntimeDiagnostic::new(error));
                        return;
                    }
                };
                let (args, named_args) = arguments;
                let Some(result) = self
                    .plan
                    .type_table()
                    .get(target.result)
                    .map(|declaration| declaration.semantic_identity())
                else {
                    let error = format!(
                        "host call {} result type is absent from the selected plan",
                        target.public_id
                    );
                    self.fiber.status = FlowFiberStatus::Failed(error.clone());
                    output.diagnostics.push(RuntimeDiagnostic::new(error));
                    return;
                };
                let id = self.next_host_call_id(&target.public_id);
                let request = crate::task::HostTaskRequest::Custom {
                    capability: crate::task::HostCapabilityId(target.capability.clone()),
                    operation: target.operation.clone(),
                    manifest_contract: target.contract,
                    args,
                    named_args,
                };
                let request = match RuntimeHostCallRequest::admit(
                    id.clone(),
                    target.producer,
                    self.generation,
                    request,
                    result,
                    target.mode,
                    target.deterministic,
                    &mut self.need_producers,
                ) {
                    Ok(request) => request,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                output.requests.host_calls.push(request);
                self.fiber.status = FlowFiberStatus::HostCall(HostCallState {
                    binding,
                    id,
                    result_type: super::HostCallResultType::Plan(target.result),
                    resume: self.resume_cursor(next_op_index),
                });
            }
            FlowOp::ProjectCall { site } => {
                self.start_project_call(site, next_op_index, output, pure_backend);
            }
            FlowOp::ApplyGroup {
                callee,
                args,
                result,
            } => {
                let resume = self.resume_cursor(next_op_index);
                let application = (|| {
                    let callee = self.evaluate_expr_with_backend(&callee, pure_backend)?;
                    let args = self.evaluate_function_call_args(&args, pure_backend)?;
                    self.start_function_value_call(
                        callee,
                        args,
                        result,
                        resume,
                        output,
                        pure_backend,
                    )
                })();
                if let Err(error) = application {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => match self.evaluate_bool_with_backend(&condition, pure_backend) {
                Ok(true) => {
                    self.advance_if_needed(next_op_index);
                    self.push_scoped_ops(then_ops);
                }
                Ok(false) => {
                    self.advance_if_needed(next_op_index);
                    self.push_scoped_ops(else_ops);
                }
                Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
            },
            FlowOp::IfLet {
                pattern,
                expr,
                guard,
                then_ops,
                else_ops,
            } => match self.evaluate_if_let_with_backend(
                &pattern,
                &expr,
                guard.as_ref(),
                pure_backend,
            ) {
                Ok(Some(bindings)) => {
                    self.advance_if_needed(next_op_index);
                    self.push_scoped_ops_with_bindings(bindings, then_ops);
                }
                Ok(None) => {
                    self.advance_if_needed(next_op_index);
                    self.push_scoped_ops(else_ops);
                }
                Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
            },
            FlowOp::Match { scrutinee, arms } => {
                match self.evaluate_expr_with_backend(&scrutinee, pure_backend) {
                    Ok(value) => {
                        self.advance_if_needed(next_op_index);
                        if let Err(error) = self.select_match_candidate(value, arms) {
                            self.fail_format_aware_eval(error, output, pure_backend);
                        }
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::Loop { result, body } => {
                self.advance_if_needed(next_op_index);
                let body = Arc::from(body);
                self.fiber.control_stack.push(FlowControlStackEntry {
                    kind: FlowControlStackEntryKind::Loop {
                        body: Arc::clone(&body),
                        result,
                    },
                });
                self.push_loop_iteration(&body);
            }
            FlowOp::LoopNext { body } => {
                self.push_loop_iteration(&body);
            }
            FlowOp::While { condition, body } => {
                match self.evaluate_bool_with_backend(&condition, pure_backend) {
                    Ok(true) => {
                        self.advance_if_needed(next_op_index);
                        let body = Arc::from(body);
                        self.fiber.control_stack.push(FlowControlStackEntry {
                            kind: FlowControlStackEntryKind::While {
                                condition: condition.clone(),
                                body: Arc::clone(&body),
                            },
                        });
                        self.push_while_iteration(condition, &body);
                    }
                    Ok(false) => self.advance_if_needed(next_op_index),
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::WhileNext { condition, body } => {
                match self.evaluate_bool_with_backend(&condition, pure_backend) {
                    Ok(true) => {
                        self.push_while_iteration(condition, &body);
                    }
                    Ok(false) => {
                        self.pop_loop_frame(output, pure_backend);
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => match self.evaluate_if_let_with_backend(
                &pattern,
                &expr,
                guard.as_ref(),
                pure_backend,
            ) {
                Ok(Some(bindings)) => {
                    self.advance_if_needed(next_op_index);
                    let body = Arc::from(body);
                    self.fiber.control_stack.push(FlowControlStackEntry {
                        kind: FlowControlStackEntryKind::WhileLet {
                            pattern: pattern.clone(),
                            expr: expr.clone(),
                            guard: guard.clone().map(Box::new),
                            body: Arc::clone(&body),
                        },
                    });
                    self.push_while_let_iteration(pattern, expr, guard, &body, bindings);
                }
                Ok(None) => self.advance_if_needed(next_op_index),
                Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
            },
            FlowOp::WhileLetNext {
                pattern,
                expr,
                guard,
                body,
            } => match self.evaluate_if_let_with_backend(
                &pattern,
                &expr,
                guard.as_ref(),
                pure_backend,
            ) {
                Ok(Some(bindings)) => {
                    self.push_while_let_iteration(pattern, expr, guard, &body, bindings);
                }
                Ok(None) => {
                    self.pop_loop_frame(output, pure_backend);
                }
                Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
            },
            FlowOp::For {
                pattern,
                source,
                evidence,
                body,
            } => {
                self.advance_if_needed(next_op_index);
                match self.evaluate_expr_with_backend(&source, pure_backend) {
                    Ok(value) => match self.runtime_iterator_from_value_with_backend(
                        value,
                        &evidence,
                        pure_backend,
                    ) {
                        Ok(iterator) => {
                            let body = Arc::from(body);
                            self.push_for_next(
                                pattern,
                                iterator,
                                evidence,
                                &body,
                                output,
                                pure_backend,
                            );
                        }
                        Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                    },
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::ForNext {
                pattern,
                iterator,
                evidence,
                body,
            } => {
                self.push_for_next(pattern, iterator, evidence, &body, output, pure_backend);
            }
            FlowOp::Thread {
                name,
                producer,
                captures,
                body,
            } => {
                let captured = captures
                    .iter()
                    .map(|local| {
                        let value = self
                            .fiber
                            .env
                            .get(*local)
                            .ok_or(RuntimeEvalError::UnknownLocal(*local))?;
                        if !value.ownership().permits_copy() {
                            return Err(RuntimeEvalError::AffineLocalCopy(*local));
                        }
                        Ok(value)
                    })
                    .collect::<Result<Vec<_>, RuntimeEvalError>>();
                let captured = match captured {
                    Ok(values) => values,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                let arguments = crate::value::RuntimeValueView::Tuple(
                    crate::value::RuntimeTupleView::Borrowed(&captured),
                );
                let label = name.as_deref().unwrap_or("anonymous");
                let request = HostTaskRequest::custom("flow_thread", "run_child", [label.into()]);
                let spec = match producer.instantiate_view(
                    self.generation,
                    arguments,
                    request,
                    16 * 1024 * 1024,
                ) {
                    Ok(spec) => spec,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                let (child, next_fiber_id) = match self.prepare_child_fiber(body, &captures) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                let accepted = self.need_producers.ensure_task_with_publication(spec, |_| {
                    self.child_fibers.push_back(child);
                    self.next_fiber_id = next_fiber_id;
                    self.run_child_next = true;
                });
                let accepted = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                self.advance_if_needed(next_op_index);
                output.requests.tasks.push(accepted);
            }
            FlowOp::Scope { identity, body } => {
                self.advance_if_needed(next_op_index);
                self.push_scoped_ops_with_identity(identity, body);
            }
            FlowOp::LetScope {
                identity,
                pattern,
                mut ops,
                value,
            } => {
                self.advance_if_needed(next_op_index);
                ops.insert(0, FlowOp::EnterScope { identity });
                ops.push(FlowOp::ExitScopeBind {
                    pattern,
                    expr: value,
                });
                self.push_ops(ops);
            }
            FlowOp::Break(expr) => {
                let value = match expr {
                    Some(expr) => match self.evaluate_expr_with_backend(&expr, pure_backend) {
                        Ok(value) => value,
                        Err(error) => {
                            self.fail_format_aware_eval(error, output, pure_backend);
                            return;
                        }
                    },
                    None => RuntimeValue::Unit,
                };
                let mut value = Some(value);
                let mut handled = self.break_nearest_loop(&mut value, output, pure_backend);
                while !handled && self.abandon_format_attempt_for_transfer() {
                    handled = self.break_nearest_loop(&mut value, output, pure_backend);
                }
                if handled {
                    self.advance_if_needed(next_op_index);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("break"), output);
                }
            }
            FlowOp::Continue => {
                let mut handled = self.continue_nearest_loop(output, pure_backend);
                while !handled && self.abandon_format_attempt_for_transfer() {
                    handled = self.continue_nearest_loop(output, pure_backend);
                }
                if handled {
                    self.advance_if_needed(next_op_index);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                }
            }
            FlowOp::Goto(target) => self.goto(&target, output, pure_backend),
            FlowOp::GotoExpr(expr) => match self.evaluate_entity_target(&expr) {
                Ok(target) => self.goto(&target, output, pure_backend),
                Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
            },
            FlowOp::Return(value) => {
                if self.has_joined_work() {
                    self.push_ops(vec![FlowOp::Return(value)]);
                    self.run_child_next = true;
                } else {
                    self.return_value(value, output, pure_backend);
                }
            }
            FlowOp::ReturnExpr(expr) => {
                if self.has_joined_work() {
                    self.push_ops(vec![FlowOp::ReturnExpr(expr)]);
                    self.run_child_next = true;
                } else {
                    match self.evaluate_expr_with_backend(&expr, pure_backend) {
                        Ok(value) => {
                            let label = runtime_value_label(&value);
                            if !self.return_function_call_value(value, output, pure_backend) {
                                self.return_value(label, output, pure_backend);
                            }
                        }
                        Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                    }
                }
            }
            FlowOp::Effect(effect) => {
                self.emit_line_effect(effect, output, pure_backend);
                if !self.apply_control_effects(output, pure_backend) {
                    self.advance_if_needed(next_op_index);
                }
            }
            FlowOp::EvaluatedEffect(effect) => {
                match self.evaluate_effect_expr(&effect, pure_backend) {
                    Ok(outcome) => {
                        if let Some(policy) = outcome.drop_policy
                            && drop_policy.replace(policy).is_some()
                        {
                            self.fail_eval(
                                crate::line_task::LineRuntimeError::InvalidActivationOperation,
                                output,
                            );
                            return;
                        }
                        if let Some(effect) = outcome.request {
                            self.emit_line_effect(effect, output, pure_backend);
                        }
                    }
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                }
                if !self.apply_control_effects(output, pure_backend) {
                    self.advance_if_needed(next_op_index);
                }
            }
            FlowOp::RegisterDefer { site, .. } => {
                self.fail_eval(RuntimeEvalError::UnknownDeferredSite { site }, output);
            }
            FlowOp::RegisterCleanup { key, effect } => {
                self.register_scope_cleanup(key, effect);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::CancelCleanup { key } => {
                self.cancel_scope_cleanup(&key);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::EnterScope { identity } => {
                self.push_scope_frame(identity);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::EnterScheduledScope { identity, token } => {
                if !token.belongs_to(self.fiber.execution, self.fiber.persistent_id)
                    || token.ordinal().get() >= self.next_scheduled_scope_sequence
                    || (token.kind() == crate::scope::RuntimeScopeFrameKind::Control
                        && !matches!(identity, RuntimeScopeIdentity::Anonymous))
                {
                    self.fail_eval(
                        crate::scope::RuntimeScopeExitError::ScheduledTargetMismatch,
                        output,
                    );
                    return;
                }
                self.push_scope_frame_with_origin(
                    identity,
                    crate::scope::RuntimeScopeFrameOrigin::Scheduled(token),
                );
                self.advance_if_needed(next_op_index);
            }
            FlowOp::ExitScheduledScope { token } => {
                let expected = Some(token);
                let valid = crate::scope::RuntimeScopeExitTarget::Frame(expected)
                    .resolve(self.active_scope_targets())
                    .is_ok();
                if !valid {
                    self.fail_eval(
                        crate::scope::RuntimeScopeExitError::ScheduledTargetMismatch,
                        output,
                    );
                    return;
                }
                self.advance_if_needed(next_op_index);
                if !self.complete_match_guard(output, pure_backend) {
                    self.pop_scope_frame(output, pure_backend)
                }
            }
            FlowOp::ExitScope => {
                if let Err(error) = self.exit_emitted_scope(output, pure_backend) {
                    self.fail_eval(error, output);
                    return;
                }
                self.advance_if_needed(next_op_index);
            }
            FlowOp::CompleteAwaitObserver => {
                let Some(state) = self.fiber.await_observer.take() else {
                    self.fiber.status = FlowFiberStatus::Failed(
                        "Await observer completed without an active Await context".to_owned(),
                    );
                    output.diagnostics.push(RuntimeDiagnostic::new(
                        "Await observer completed without an active Await context".to_owned(),
                    ));
                    return;
                };
                self.fiber.status = FlowFiberStatus::NeedWaiting(state);
            }
            FlowOp::ExitScopeBind { pattern, expr } => {
                let value = match self.evaluate_expr_with_backend(&expr, pure_backend) {
                    Ok(value) => value,
                    Err(error) => {
                        self.fail_format_aware_eval(error, output, pure_backend);
                        return;
                    }
                };
                if let Err(error) = self.exit_emitted_scope(output, pure_backend) {
                    self.fail_eval(error, output);
                    return;
                }
                self.bind_value(&pattern, value, output);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::Noop => {
                self.advance_if_needed(next_op_index);
            }
        }
    }

    pub(in crate::engine) fn has_active_project_call(&self) -> bool {
        self.fiber
            .control_stack
            .iter()
            .any(|entry| matches!(entry.kind, FlowControlStackEntryKind::FunctionCall(_)))
    }

    fn start_project_call(
        &mut self,
        site: crate::runtime_id::RuntimeProjectCallSiteId,
        next_op_index: Option<usize>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let owner = Arc::clone(&self.plan);
        let Some(row) = owner.project_call_sites().get(site) else {
            self.fiber.status =
                FlowFiberStatus::Failed(format!("missing project-call site {site}"));
            return;
        };
        let plan = row.plan();
        let prepared = (|| {
            let value = self.evaluate_expr_with_backend(plan.callee(), pure_backend)?;
            let RuntimeValue::Callable(callable) = value else {
                return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                    &value,
                )));
            };
            callable
                .validate_for_owner(&crate::task::RuntimeProgramOwner::Plan(Arc::clone(&owner)))?;
            if callable.state() != plan.state() {
                return Err(crate::value::RuntimeCallableValueError::UnexpectedState {
                    expected: plan.state(),
                    actual: callable.state(),
                }
                .into());
            }
            let operands = self.evaluate_project_call_operands(plan, pure_backend)?;
            self.inspect_project_call_materialization(plan, &operands)?;
            let (arguments, attached) = self.materialize_project_call_owned(plan, operands);
            Ok::<_, RuntimeEvalError>((callable, arguments, attached))
        })();
        let (callable, arguments, attached) = match prepared {
            Ok(values) => values,
            Err(error) => {
                self.fail_format_aware_eval(error, output, pure_backend);
                return;
            }
        };
        let resume = self.resume_cursor(next_op_index);
        if let Err(error) = self.start_callable_group(
            callable,
            arguments,
            attached,
            row.result().clone(),
            resume,
            output,
            pure_backend,
        ) {
            self.fail_format_aware_eval(error, output, pure_backend);
        }
    }
    fn evaluate_project_call_operands(
        &mut self,
        plan: &crate::plan::RuntimeProjectCallPlan,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<Vec<RuntimeValue>>, RuntimeEvalError> {
        plan.operands()
            .iter()
            .map(|operand| {
                let value = self.evaluate_expr_with_backend(operand.value(), pure_backend)?;
                match operand.mode() {
                    crate::value::RuntimeCallArgumentMode::Value => Ok(vec![value]),
                    crate::value::RuntimeCallArgumentMode::Spread => {
                        crate::value::runtime_value_into_sequence_values(value).map_err(|value| {
                            RuntimeEvalError::InvalidSpread(runtime_value_label(&value))
                        })
                    }
                }
            })
            .collect()
    }

    fn inspect_project_call_materialization(
        &self,
        plan: &crate::plan::RuntimeProjectCallPlan,
        operands: &[Vec<RuntimeValue>],
    ) -> Result<(), RuntimeEvalError> {
        let source = |index: u32| {
            usize::try_from(index)
                .ok()
                .and_then(|index| operands.get(index))
                .map(Vec::as_slice)
                .ok_or_else(|| {
                    RuntimeEvalError::InvalidSpread("project-call source is absent".to_owned())
                })
        };
        let mut seen = std::collections::BTreeSet::new();
        for row in plan.ordinary() {
            match row {
                RuntimeProjectCallOrdinaryMaterialization::Fixed(row) => {
                    let values = source(row.source_index())?;
                    if !seen.insert(row.source_index()) || values.len() != 1 {
                        return Err(RuntimeEvalError::InvalidSpread(
                            "fixed project-call source must be unique and singular".to_owned(),
                        ));
                    }
                    self.require_project_call_value(row.binding_ty(), &values[0])?;
                }
                RuntimeProjectCallOrdinaryMaterialization::Rest(row) => {
                    for &index in row.source_indices() {
                        if !seen.insert(index) {
                            return Err(RuntimeEvalError::InvalidSpread(
                                "project-call source is referenced twice".to_owned(),
                            ));
                        }
                        for value in source(index)? {
                            self.require_project_call_value(row.abi_ty(), value)?;
                        }
                    }
                }
            }
        }
        if let Some(index) = plan.attached().and_then(|row| row.source_index()) {
            let values = source(index)?;
            if !seen.insert(index) || values.len() != 1 {
                return Err(RuntimeEvalError::InvalidSpread(
                    "attached project-call source must be unique and singular".to_owned(),
                ));
            }
            let expected = plan
                .operands()
                .get(index as usize)
                .expect("sealed attached source index names an operand")
                .value()
                .ty();
            self.require_project_call_value(expected, &values[0])?;
        }
        if seen.len() != operands.len() {
            return Err(RuntimeEvalError::InvalidSpread(
                "project-call operand has no owned destination".to_owned(),
            ));
        }
        Ok(())
    }

    fn materialize_project_call_owned(
        &self,
        plan: &crate::plan::RuntimeProjectCallPlan,
        operands: Vec<Vec<RuntimeValue>>,
    ) -> (Vec<RuntimeValue>, Option<RuntimeValue>) {
        let mut sources = operands.into_iter().map(Some).collect::<Vec<_>>();
        let mut take = |index: u32| {
            sources
                .get_mut(index as usize)
                .and_then(Option::take)
                .expect("borrowed project-call proof sealed one source destination")
        };
        let mut arguments = Vec::with_capacity(plan.ordinary().len());
        for row in plan.ordinary() {
            match row {
                RuntimeProjectCallOrdinaryMaterialization::Fixed(row) => {
                    let value = take(row.source_index())
                        .pop()
                        .expect("fixed project-call source was singular");
                    arguments.push(value);
                }
                RuntimeProjectCallOrdinaryMaterialization::Rest(row) => {
                    let mut elements = Vec::new();
                    for &index in row.source_indices() {
                        elements.extend(take(index));
                    }
                    arguments.push(crate::value::runtime_sequence_values(elements));
                }
            }
        }
        let attached = plan
            .attached()
            .and_then(|row| row.source_index())
            .map(|index| {
                take(index)
                    .pop()
                    .expect("attached project-call source was singular")
            });
        assert!(sources.iter().all(Option::is_none));
        (arguments, attached)
    }

    fn require_project_call_value(
        &self,
        expected: crate::runtime_id::RuntimePlanTypeId,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeEvalError> {
        if self.plan.value_matches_type(expected, value)? {
            Ok(())
        } else {
            Err(RuntimeEvalError::InvalidExpressionType(expected))
        }
    }

    pub(super) fn evaluate_host_call_arguments(
        &mut self,
        arguments: &[RuntimeHostArgumentTemplate],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<
        (
            Vec<crate::value::RuntimePayload>,
            Vec<NamedHostArg<crate::value::RuntimePayload>>,
        ),
        String,
    > {
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for argument in arguments {
            let value = self
                .evaluate_expr_with_backend(argument.value(), pure_backend)
                .map_err(|error| error.to_string())?;
            match argument {
                RuntimeHostArgumentTemplate::Positional(..) => {
                    positional.push(crate::value::RuntimePayload::from(value));
                }
                RuntimeHostArgumentTemplate::Named(_, argument) => named.push(NamedHostArg {
                    name: argument.name.clone(),
                    value: crate::value::RuntimePayload::from(value),
                }),
                RuntimeHostArgumentTemplate::Spread(..) => {
                    let values = crate::value::runtime_value_into_sequence_values(value).map_err(
                        |value| {
                            format!(
                                "spread host argument requires a tuple or bracket sequence, found {}",
                                runtime_value_label(&value)
                            )
                        },
                    )?;
                    positional.extend(values.into_iter().map(crate::value::RuntimePayload::from));
                }
            }
        }
        Ok((positional, named))
    }

    pub(super) fn bind_value(
        &mut self,
        pattern: &RuntimePattern,
        value: RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        match self.try_bind_pattern_owned(pattern, value) {
            Ok(None) => {}
            Ok(Some(value)) => self.fail_eval(
                RuntimeEvalError::PatternMismatch(runtime_value_label(&value)),
                output,
            ),
            Err(error) => self.fail_eval(error, output),
        }
    }

    pub(super) fn advance_if_needed(&mut self, next_op_index: Option<usize>) {
        if let Some(next_op_index) = next_op_index
            && let Some(cursor) = self.fiber.cursor.as_mut()
        {
            cursor.op_index = next_op_index;
        }
    }

    pub(super) fn next_host_call_id(&mut self, public_id: &str) -> RuntimeHostCallId {
        let id = self.preview_host_call_id(public_id);
        self.advance_host_call_sequence();
        id
    }

    pub(super) fn preview_host_call_id(&self, public_id: &str) -> RuntimeHostCallId {
        let sequence = self.next_host_call_sequence;
        RuntimeHostCallId(if sequence == 0 {
            public_id.to_owned()
        } else {
            format!("{public_id}.{sequence}")
        })
    }

    pub(super) fn advance_host_call_sequence(&mut self) {
        self.next_host_call_sequence = self.next_host_call_sequence.saturating_add(1);
    }

    fn resume_cursor(&self, next_op_index: Option<usize>) -> Option<FlowCursor> {
        self.fiber.cursor.map(|cursor| {
            let mut cursor = cursor;
            if let Some(next_op_index) = next_op_index {
                cursor.op_index = next_op_index;
            }
            cursor
        })
    }

    pub(super) fn push_ops(&mut self, ops: Vec<FlowOp>) {
        self.fiber.pending_ops.reserve(ops.len());
        for op in ops.into_iter().rev() {
            self.fiber.pending_ops.push_front(op);
        }
    }

    pub(super) fn push_scope_frame(&mut self, identity: RuntimeScopeIdentity) {
        self.push_scope_frame_with_origin(
            identity,
            crate::scope::RuntimeScopeFrameOrigin::EmittedLexical,
        );
    }

    fn push_scope_frame_with_origin(
        &mut self,
        identity: RuntimeScopeIdentity,
        origin: crate::scope::RuntimeScopeFrameOrigin,
    ) {
        self.fiber.env.push_scope_with_identity(identity);
        self.fiber.control_stack.push(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope {
                origin,
                cleanups: Vec::new(),
                match_guard: None,
            },
        });
    }

    fn allocate_scheduled_scope(
        &mut self,
        kind: crate::scope::RuntimeScopeFrameKind,
    ) -> Option<crate::scope::RuntimeScheduledScopeToken> {
        let ordinal = std::num::NonZeroU64::new(self.next_scheduled_scope_sequence)?;
        let Some(next) = self.next_scheduled_scope_sequence.checked_add(1) else {
            self.fiber.status = FlowFiberStatus::Failed(
                crate::scope::RuntimeScopeExitError::IdentityCapacityExhausted.to_string(),
            );
            return None;
        };
        self.next_scheduled_scope_sequence = next;
        Some(crate::scope::RuntimeScheduledScopeToken::from_runtime(
            self.fiber.execution,
            self.fiber.persistent_id,
            ordinal,
            kind,
        ))
    }
    pub(super) fn push_control_scope_frame(
        &mut self,
    ) -> Option<crate::scope::RuntimeScheduledScopeToken> {
        let token = self.allocate_scheduled_scope(crate::scope::RuntimeScopeFrameKind::Control)?;
        self.push_scope_frame_with_origin(
            RuntimeScopeIdentity::Anonymous,
            crate::scope::RuntimeScopeFrameOrigin::Scheduled(token),
        );
        Some(token)
    }
    fn active_scope_targets(
        &self,
    ) -> impl Iterator<
        Item = (
            Option<crate::scope::RuntimeScheduledScopeToken>,
            crate::scope::RuntimeScopeFrameKind,
        ),
    > + '_ {
        self.fiber
            .control_stack
            .iter()
            .rev()
            .map_while(|frame| match &frame.kind {
                FlowControlStackEntryKind::Scope { origin, .. } => {
                    Some((origin.token(), origin.kind()))
                }
                _ => None,
            })
    }
    pub(super) fn exit_emitted_scope(
        &mut self,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), crate::scope::RuntimeScopeExitError> {
        let exit = crate::scope::RuntimeScopeExitTarget::EmittedLexical
            .resolve(self.active_scope_targets())?;
        for _ in exit.targets() {
            self.pop_scope_frame(output, backend);
        }
        Ok(())
    }

    pub(super) fn register_scope_cleanup(
        &mut self,
        key: impl Into<String>,
        effect: LineEffectRequest,
    ) {
        let cleanup = FlowScopeCleanup::new(key, effect);
        if let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope { cleanups, .. },
        }) = self
            .fiber
            .control_stack
            .iter_mut()
            .rev()
            .find(|entry| matches!(&entry.kind, FlowControlStackEntryKind::Scope { .. }))
        {
            cleanups.push(cleanup);
        } else {
            self.fiber.root_cleanups.push(cleanup);
        }
    }

    pub(super) fn cancel_scope_cleanup(&mut self, key: &str) {
        self.fiber
            .root_cleanups
            .retain(|cleanup| cleanup.key != key);
        for entry in &mut self.fiber.control_stack {
            if let FlowControlStackEntryKind::Scope { cleanups, .. } = &mut entry.kind {
                cleanups.retain(|cleanup| cleanup.key != key);
            }
        }
    }

    pub(super) fn push_scoped_ops(&mut self, ops: Vec<FlowOp>) {
        self.push_owned_scoped_ops(ops, None);
    }

    pub(super) fn push_scoped_ops_with_identity(
        &mut self,
        identity: RuntimeScopeIdentity,
        ops: Vec<FlowOp>,
    ) {
        self.push_owned_scoped_ops_with_identity(ops, None, identity);
    }

    pub(super) fn push_scoped_ops_with_bindings(
        &mut self,
        bindings: Vec<RuntimeLocalBinding>,
        ops: Vec<FlowOp>,
    ) {
        if bindings.is_empty() && ops.is_empty() {
            return;
        }
        let prefix = (!bindings.is_empty()).then_some(FlowOp::Bind(bindings));
        self.push_owned_scoped_ops(ops, prefix);
    }

    pub(super) fn push_await_observer_ops(
        &mut self,
        bindings: Vec<RuntimeLocalBinding>,
        ops: &[FlowOp],
    ) {
        let prefix = (!bindings.is_empty()).then_some(FlowOp::Bind(bindings));
        self.push_borrowed_scoped_ops(ops, prefix, Some(FlowOp::CompleteAwaitObserver));
    }

    pub(super) fn push_loop_iteration(&mut self, body: &Arc<[FlowOp]>) {
        let tail = FlowOp::LoopNext {
            body: Arc::clone(body),
        };
        self.push_borrowed_scoped_ops(body.as_ref(), None, Some(tail));
    }

    pub(super) fn push_while_iteration(&mut self, condition: RuntimeExpr, body: &Arc<[FlowOp]>) {
        let tail = FlowOp::WhileNext {
            condition,
            body: Arc::clone(body),
        };
        self.push_borrowed_scoped_ops(body.as_ref(), None, Some(tail));
    }

    pub(super) fn push_while_let_iteration(
        &mut self,
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: &Arc<[FlowOp]>,
        bindings: Vec<RuntimeLocalBinding>,
    ) {
        let prefix = (!bindings.is_empty()).then_some(FlowOp::Bind(bindings));
        let tail = FlowOp::WhileLetNext {
            pattern,
            expr,
            guard,
            body: Arc::clone(body),
        };
        self.push_borrowed_scoped_ops(body.as_ref(), prefix, Some(tail));
    }

    pub(super) fn push_for_next(
        &mut self,
        pattern: RuntimePattern,
        mut iterator: RuntimeIterator,
        evidence: RuntimeIteratorEvidence,
        body: &Arc<[FlowOp]>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let item = match self.next_runtime_iterator_item(&mut iterator, pure_backend) {
            Ok(Some(item)) => item,
            Ok(None) => return,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        self.push_for_item(pattern, iterator, evidence, body, item, output);
    }

    fn push_for_item(
        &mut self,
        pattern: RuntimePattern,
        iterator: RuntimeIterator,
        evidence: RuntimeIteratorEvidence,
        body: &Arc<[FlowOp]>,
        item: RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(token) = self.push_control_scope_frame() else {
            return;
        };
        match self.try_bind_pattern_owned(&pattern, item) {
            Ok(None) => {}
            Ok(Some(item)) => {
                self.fail_eval(
                    RuntimeEvalError::PatternMismatch(runtime_value_label(&item)),
                    output,
                );
                return;
            }
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        }
        let tail = FlowOp::ForNext {
            pattern,
            iterator,
            evidence,
            body: Arc::clone(body),
        };
        self.push_borrowed_ops_with_exit(body.as_ref(), Some(tail), token);
    }

    fn runtime_iterator_from_value_with_backend(
        &mut self,
        value: RuntimeValue,
        evidence: &RuntimeIteratorEvidence,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeIterator, RuntimeEvalError> {
        if let RuntimeIteratorEvidence::Witness(witness) = evidence {
            return match &witness.executable {
                RuntimeIteratorWitnessExecutable::TraitCalls { into_iter, next } => {
                    let outcome = self.evaluate_trait_method_values(
                        *into_iter,
                        RuntimeReceiverMode::Owned,
                        value,
                        Vec::new(),
                        pure_backend,
                    )?;
                    Ok(RuntimeIterator::witness(outcome.value, *next))
                }
                RuntimeIteratorWitnessExecutable::IdentityIntoIterator { next } => {
                    Ok(RuntimeIterator::witness(value, *next))
                }
            };
        }
        RuntimeIterator::from_value_with_evidence(value, evidence)
            .map_err(|value| RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value)))
    }

    fn next_runtime_iterator_item(
        &mut self,
        iterator: &mut RuntimeIterator,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let RuntimeIterator::Witness { state, next } = iterator else {
            return Ok(iterator.next());
        };
        let outcome = self.evaluate_trait_method_values(
            *next,
            RuntimeReceiverMode::MutRef,
            std::mem::replace(state.as_mut(), RuntimeValue::Unit),
            Vec::new(),
            pure_backend,
        )?;
        if let Some(updated_receiver) = outcome.updated_receiver {
            **state = updated_receiver;
        }
        let label = runtime_value_label(&outcome.value);
        match outcome.value.try_into_builtin_variant_case() {
            Ok((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => Ok(None),
            Ok((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => Ok(Some(value)),
            _ => Err(RuntimeEvalError::ExpectedBracketSeq(format!(
                "Iterator::next expected Option, found {label}"
            ))),
        }
    }

    fn push_owned_scoped_ops(&mut self, ops: Vec<FlowOp>, prefix: Option<FlowOp>) {
        if ops.is_empty() && prefix.is_none() {
            return;
        }
        let Some(token) =
            self.allocate_scheduled_scope(crate::scope::RuntimeScopeFrameKind::Control)
        else {
            return;
        };
        self.fiber
            .pending_ops
            .push_front(FlowOp::ExitScheduledScope { token });
        for op in ops.into_iter().rev() {
            self.fiber.pending_ops.push_front(op)
        }
        if let Some(prefix) = prefix {
            self.fiber.pending_ops.push_front(prefix)
        }
        self.fiber
            .pending_ops
            .push_front(FlowOp::EnterScheduledScope {
                identity: RuntimeScopeIdentity::Anonymous,
                token,
            });
    }

    fn push_owned_scoped_ops_with_identity(
        &mut self,
        ops: Vec<FlowOp>,
        prefix: Option<FlowOp>,
        identity: RuntimeScopeIdentity,
    ) {
        if ops.is_empty() && prefix.is_none() {
            return;
        }
        let Some(token) =
            self.allocate_scheduled_scope(crate::scope::RuntimeScopeFrameKind::EmittedLexical)
        else {
            return;
        };
        self.fiber
            .pending_ops
            .reserve(ops.len() + usize::from(prefix.is_some()) + 2);
        self.fiber
            .pending_ops
            .push_front(FlowOp::ExitScheduledScope { token });
        for op in ops.into_iter().rev() {
            self.fiber.pending_ops.push_front(op)
        }
        if let Some(prefix) = prefix {
            self.fiber.pending_ops.push_front(prefix)
        }
        self.fiber
            .pending_ops
            .push_front(FlowOp::EnterScheduledScope { identity, token });
    }

    fn push_borrowed_scoped_ops(
        &mut self,
        ops: &[FlowOp],
        prefix: Option<FlowOp>,
        tail: Option<FlowOp>,
    ) {
        if ops.is_empty() && prefix.is_none() && tail.is_none() {
            return;
        }
        self.fiber
            .pending_ops
            .reserve(ops.len() + usize::from(prefix.is_some()) + usize::from(tail.is_some()) + 2);
        if let Some(tail) = tail {
            self.fiber.pending_ops.push_front(tail);
        }
        let Some(token) =
            self.allocate_scheduled_scope(crate::scope::RuntimeScopeFrameKind::Control)
        else {
            return;
        };
        self.fiber
            .pending_ops
            .push_front(FlowOp::ExitScheduledScope { token });
        for op in ops.iter().rev().cloned() {
            self.fiber.pending_ops.push_front(op);
        }
        if let Some(prefix) = prefix {
            self.fiber.pending_ops.push_front(prefix);
        }
        self.fiber
            .pending_ops
            .push_front(FlowOp::EnterScheduledScope {
                identity: RuntimeScopeIdentity::Anonymous,
                token,
            });
    }

    fn push_borrowed_ops_with_exit(
        &mut self,
        ops: &[FlowOp],
        tail: Option<FlowOp>,
        token: crate::scope::RuntimeScheduledScopeToken,
    ) {
        self.fiber
            .pending_ops
            .reserve(ops.len() + usize::from(tail.is_some()) + 1);
        if let Some(tail) = tail {
            self.fiber.pending_ops.push_front(tail);
        }
        self.fiber
            .pending_ops
            .push_front(FlowOp::ExitScheduledScope { token });
        for op in ops.iter().rev().cloned() {
            self.fiber.pending_ops.push_front(op);
        }
    }

    pub(super) fn cancel_scheduled_scope_close(
        &mut self,
        origin: crate::scope::RuntimeScopeFrameOrigin,
    ) {
        if let crate::scope::RuntimeScopeFrameOrigin::Scheduled(token) = origin {
            self.fiber.pending_ops.retain(
                |op| !matches!(op,FlowOp::ExitScheduledScope {token:queued} if *queued==token),
            );
        }
    }

    pub(super) fn pop_scope_frame(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if !matches!(
            self.fiber.control_stack.last(),
            Some(FlowControlStackEntry {
                kind: FlowControlStackEntryKind::Scope { .. }
            })
        ) {
            return;
        }
        let Some(FlowControlStackEntry {
            kind:
                FlowControlStackEntryKind::Scope {
                    origin, cleanups, ..
                },
        }) = self.fiber.control_stack.pop()
        else {
            return;
        };
        self.cancel_scheduled_scope_close(origin);
        self.fiber.env.pop_scope();
        self.emit_scope_cleanups(cleanups, output, pure_backend);
    }

    pub(super) fn pop_scope_frames_until_loop(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        while matches!(
            self.fiber.control_stack.last(),
            Some(FlowControlStackEntry {
                kind: FlowControlStackEntryKind::Scope { .. }
            })
        ) {
            self.pop_scope_frame(output, pure_backend);
        }
    }

    pub(super) fn pop_loop_frame(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Option<FlowControlStackEntryKind> {
        self.pop_scope_frames_until_loop(output, pure_backend);
        let is_loop = self.fiber.control_stack.last().is_some_and(|entry| {
            matches!(
                entry.kind,
                FlowControlStackEntryKind::Loop { .. }
                    | FlowControlStackEntryKind::While { .. }
                    | FlowControlStackEntryKind::WhileLet { .. }
            )
        });
        if is_loop {
            self.fiber.control_stack.pop().map(|entry| entry.kind)
        } else {
            None
        }
    }

    pub(super) fn discard_pending_until_loop_next(&mut self) {
        while let Some(op) = self.fiber.pending_ops.pop_front() {
            if matches!(
                op,
                FlowOp::LoopNext { .. } | FlowOp::WhileNext { .. } | FlowOp::WhileLetNext { .. }
            ) {
                break;
            }
        }
    }

    pub(super) fn break_nearest_loop(
        &mut self,
        value: &mut Option<RuntimeValue>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        self.discard_pending_until_loop_next();
        let Some(kind) = self.pop_loop_frame(output, pure_backend) else {
            return false;
        };
        self.fiber.await_observer = None;
        match kind {
            FlowControlStackEntryKind::Loop {
                result: Some(pattern),
                ..
            } => self.bind_value(
                &pattern,
                value.take().expect("selected break value remains owned"),
                output,
            ),
            FlowControlStackEntryKind::Loop { result: None, .. } => {}
            FlowControlStackEntryKind::While { .. }
            | FlowControlStackEntryKind::WhileLet { .. } => {
                if value.as_ref() != Some(&RuntimeValue::Unit) {
                    self.fail_eval(RuntimeEvalError::BreakValueOutsideValueLoop, output);
                }
            }
            FlowControlStackEntryKind::Scope { .. } => return false,
            FlowControlStackEntryKind::FormatAttempt(_) => return false,
            FlowControlStackEntryKind::FunctionCall(_) => return false,
        }
        true
    }

    pub(super) fn continue_nearest_loop(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        self.pop_scope_frames_until_loop(output, pure_backend);
        self.discard_pending_until_loop_next();
        let Some(kind) = self
            .fiber
            .control_stack
            .last()
            .map(|frame| frame.kind.clone())
        else {
            return false;
        };
        self.fiber.await_observer = None;
        match kind {
            FlowControlStackEntryKind::Loop { body, .. } => self.push_loop_iteration(&body),
            FlowControlStackEntryKind::While { condition, body } => {
                self.push_ops(vec![FlowOp::WhileNext { condition, body }]);
            }
            FlowControlStackEntryKind::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                self.push_ops(vec![FlowOp::WhileLetNext {
                    pattern,
                    expr,
                    guard: guard.map(|guard| *guard),
                    body,
                }]);
            }
            FlowControlStackEntryKind::Scope { .. } => {
                self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                return false;
            }
            FlowControlStackEntryKind::FormatAttempt(_) => return false,
            FlowControlStackEntryKind::FunctionCall(_) => {
                self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                return false;
            }
        }
        true
    }

    pub(super) fn drain_root_cleanups(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let cleanups = std::mem::take(&mut self.fiber.root_cleanups);
        self.emit_scope_cleanups(cleanups, output, pure_backend);
    }

    pub(super) fn emit_scope_cleanups(
        &mut self,
        mut cleanups: Vec<FlowScopeCleanup>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        while let Some(cleanup) = cleanups.pop() {
            self.emit_line_effect(cleanup.effect, output, pure_backend);
        }
    }
}

#[cfg(test)]
#[path = "flow/iterator_tests.rs"]
mod iterator_tests;

#[cfg(test)]
mod ownership_tests {
    use super::Engine;
    use crate::pattern::RuntimeSemanticTypeId;
    use crate::plan::{
        RuntimePlanBuilder, RuntimePlanSequenceKind, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed, RuntimeProjectCallOperand, RuntimeProjectCallOrdinaryMaterialization,
        RuntimeProjectCallPlan, RuntimeProjectCallRestMaterialization,
    };
    use crate::runtime_id::{RuntimeCallableStateId, RuntimeLocalDeclarationId};
    use crate::task::NeedId;
    use crate::value::{
        RuntimeCallArgumentMode, RuntimeExpr, RuntimeExprKind, RuntimeLocalRead,
        RuntimeLocalReadMode, RuntimeValue, runtime_value_into_sequence_values,
    };
    use std::num::NonZeroU32;

    #[test]
    fn project_call_rest_materialization_moves_two_affine_sources_once() {
        let unit = RuntimeSemanticTypeId::from_bytes([0x81; 32]);
        let need = RuntimeSemanticTypeId::from_bytes([0x82; 32]);
        let sequence = RuntimeSemanticTypeId::from_bytes([0x83; 32]);
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                    RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(unit)),
                    RuntimePlanTypeSeed::new(
                        sequence,
                        RuntimePlanTypeProjection::Sequence {
                            kind: RuntimePlanSequenceKind::Vec,
                            item: need,
                        },
                    ),
                ],
                [],
            )
            .expect("affine rest types");
        let plan = builder.finish().expect("closed type table");
        let unit_ty = plan.type_table().id_for_semantic(unit).expect("unit");
        let need_ty = plan.type_table().id_for_semantic(need).expect("Need");
        let sequence_ty = plan
            .type_table()
            .id_for_semantic(sequence)
            .expect("Vec<Need>");
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        let operand = || {
            RuntimeProjectCallOperand::from_admitted_parts(
                RuntimeExpr::from_admitted_parts(
                    need_ty,
                    RuntimeExprKind::Local(RuntimeLocalRead::from_admitted_parts(
                        local,
                        RuntimeLocalReadMode::Move,
                    )),
                ),
                RuntimeCallArgumentMode::Value,
            )
        };
        let call = RuntimeProjectCallPlan::try_from_admitted_parts(
            RuntimeExpr::from_admitted_parts(unit_ty, RuntimeExprKind::Value(RuntimeValue::Unit)),
            RuntimeCallableStateId::from_zero_based(0).expect("state id"),
            0,
            vec![operand(), operand()].into_boxed_slice(),
            vec![RuntimeProjectCallOrdinaryMaterialization::Rest(
                RuntimeProjectCallRestMaterialization::from_admitted_parts(
                    0,
                    need_ty,
                    sequence_ty,
                    Box::new([0, 1]),
                ),
            )]
            .into_boxed_slice(),
            None,
        )
        .expect("each physical source has one destination");
        let engine = Engine::new(plan);
        let first = crate::tests::reusable_need("need.project.rest.first");
        let second = crate::tests::reusable_need("need.project.rest.second");
        let operands = vec![
            vec![RuntimeValue::NeedHandle(first.clone())],
            vec![RuntimeValue::NeedHandle(second.clone())],
        ];
        engine
            .inspect_project_call_materialization(&call, &operands)
            .expect("borrowed affine source preflight");
        let (arguments, attached) = engine.materialize_project_call_owned(&call, operands);
        assert!(attached.is_none());
        let mut arguments = arguments.into_iter();
        let sequence = arguments.next().expect("one logical rest argument");
        assert!(arguments.next().is_none());
        let values = runtime_value_into_sequence_values(sequence).expect("owned rest pack");
        assert_eq!(
            values,
            vec![
                RuntimeValue::NeedHandle(first),
                RuntimeValue::NeedHandle(second)
            ]
        );
    }
}

#[cfg(test)]
#[path = "flow/scope_tests.rs"]
mod scope_tests;
