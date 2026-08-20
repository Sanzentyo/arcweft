use super::{
    AwaitState, ChoiceState, DialogueState, Engine, FlowControlStackEntry,
    FlowControlStackEntryKind, FlowCursor, FlowEvent, FlowFiberStatus, FlowOp, FlowScopeCleanup,
    HostCallState, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr, RuntimeIterator,
    RuntimePattern, RuntimeStepInput, RuntimeStepOutput, RuntimeValue, runtime_value_label,
};
use crate::effect::LineEffectRequest;
use crate::line_task::{LineTaskLiveState, progress_live_line_task_group};
use crate::pattern::pattern_binding_capacity;
use crate::plan::{RuntimeIteratorEvidence, RuntimeIteratorWitnessExecutable, RuntimeReceiverMode};
use crate::pure::RuntimeCallBackend;
use crate::step::{RuntimeHostCallId, RuntimeHostCallRequest};
use crate::task::{
    CancelScopeId, HostTaskRequest, NamedHostArg, RuntimeHostArgumentTemplate, TaskClass, TaskId,
    TaskKey, TaskPolicy, TaskPriority, TaskSpec,
};
use crate::time::LogicalDuration;
use crate::value::RuntimeLocalBinding;
use std::sync::Arc;

impl Engine {
    pub(super) fn dialogue_marks_for_input(
        content: &crate::plan::RuntimeDialogueContentPlan,
        input: &RuntimeStepInput,
    ) -> std::collections::BTreeSet<crate::runtime_id::RuntimeDialogueMarkId> {
        input
            .input_events
            .iter()
            .filter_map(|event| {
                let name = crate::step::input_event_trigger_name(event)?;
                let label = if name == "mark" {
                    crate::step::input_event_text_payload(event)?
                } else {
                    name.strip_prefix("mark:")?
                };
                content.resolve_mark_label(label)
            })
            .collect()
    }

    // Keep the opcode dispatcher contiguous while the Phase 1 runtime surface is
    // still changing; extracting each arm now would obscure grammar coverage.
    #[allow(clippy::too_many_lines)]
    pub(super) fn step_flow(
        &mut self,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let (op, next_op_index) = if let Some(op) = self.fiber.pending_ops.pop_front() {
            (op, None)
        } else {
            let Some(cursor) = self.fiber.cursor.as_ref() else {
                return;
            };
            let Some(op) = self
                .flow_at_cursor(cursor)
                .and_then(|flow| flow.ops.get(cursor.op_index))
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
            FlowOp::LetElse {
                pattern,
                expr,
                else_ops,
            } => {
                match self
                    .evaluate_expr_with_backend(&expr, pure_backend)
                    .and_then(|value| {
                        self.try_bind_pattern(&pattern, &value)
                            .map(|matched| (matched, value))
                    }) {
                    Ok((true, _)) => self.advance_if_needed(next_op_index),
                    Ok((false, value)) => {
                        self.advance_if_needed(next_op_index);
                        self.push_ops(else_ops);
                        output.diagnostics.push(RuntimeDiagnostic::new(format!(
                            "let-else pattern did not match {}",
                            runtime_value_label(&value)
                        )));
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::AssignNominalField { base, field, value } => {
                match self.evaluate_expr_with_backend(&value, pure_backend) {
                    Ok(value) => match self.fiber.env.set_record_field(base, field, value) {
                        Ok(()) => self.advance_if_needed(next_op_index),
                        Err(target) => self.fail_eval(
                            RuntimeEvalError::InvalidFieldAssignment {
                                field: field.zero_based().to_string(),
                                value: runtime_value_label(&target),
                            },
                            output,
                        ),
                    },
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::Dialogue { content } => {
                let Some(content_plan) = self.plan.dialogue_content().get(content).cloned() else {
                    self.fiber.status =
                        FlowFiberStatus::Failed(format!("missing dialogue content plan {content}"));
                    return;
                };
                let mut values = Vec::with_capacity(content_plan.values().len());
                for site in content_plan.values() {
                    match self.evaluate_dialogue_site(site.function(), pure_backend) {
                        Ok(value) => values.push(crate::plan::RuntimeDialogueValueBinding {
                            slot: site.slot(),
                            value,
                        }),
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    }
                }
                let line = content_plan.line().clone();
                output.flow_events.push(FlowEvent::DialogueLine {
                    line: line.clone(),
                    values: values.into_boxed_slice(),
                });
                let elapsed = LogicalDuration::default();
                let task_group = content_plan.line_task_group();
                let (captures, line_task) = match task_group {
                    Some(task_group) => {
                        let Some(group) = self
                            .plan
                            .line_task_groups()
                            .get(task_group.index())
                            .cloned()
                        else {
                            self.fiber.status = FlowFiberStatus::Failed(format!(
                                "dialogue content references missing line task group {task_group}"
                            ));
                            return;
                        };
                        let captures = match self.capture_line_task_locals(&group) {
                            Ok(captures) => captures,
                            Err(message) => {
                                self.fiber.status = FlowFiberStatus::Failed(message);
                                return;
                            }
                        };
                        let activation_id = self.next_line_task_activation;
                        self.next_line_task_activation =
                            self.next_line_task_activation.saturating_add(1);
                        let mut line_task = LineTaskLiveState::new(&group, activation_id);
                        let marks = Self::dialogue_marks_for_input(&content_plan, input);
                        let activation =
                            progress_live_line_task_group(&group, elapsed, &marks, &mut line_task);
                        self.spawn_line_task_commands(&group, activation, &captures);
                        (captures, Some(line_task))
                    }
                    None => (Box::<[RuntimeLocalBinding]>::default(), None),
                };
                self.fiber.status = FlowFiberStatus::Dialogue(DialogueState {
                    line,
                    content,
                    task_group,
                    resume: self.resume_cursor(next_op_index),
                    captures,
                    line_task,
                    elapsed,
                });
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
                output.flow_events.push(FlowEvent::AwaitStarted {
                    need: target.need.clone(),
                    task: target.task.clone(),
                });
                let Some(task) = self.await_task_spec(&target, output, pure_backend) else {
                    return;
                };
                output.requests.tasks.push(task);
                let observed_through = self.task_publications.get(&target.task).copied();
                self.fiber.status = FlowFiberStatus::Waiting(AwaitState {
                    binding,
                    target,
                    observers,
                    resume: self.resume_cursor(next_op_index),
                    observed_through,
                    queued: std::collections::VecDeque::new(),
                });
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
                let id = self.next_host_call_id(&target.public_id);
                output.requests.host_calls.push(RuntimeHostCallRequest {
                    id: id.clone(),
                    public_id: target.public_id.clone(),
                    capability: target.capability.clone(),
                    operation: target.operation.clone(),
                    args,
                    named_args,
                    mode: target.mode,
                    deterministic: target.deterministic,
                });
                self.fiber.status = FlowFiberStatus::HostCall(HostCallState {
                    binding,
                    target,
                    id,
                    resume: self.resume_cursor(next_op_index),
                });
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
                Err(error) => self.fail_eval(error, output),
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
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Match { scrutinee, arms } => {
                match self.evaluate_match_with_backend(&scrutinee, arms, pure_backend) {
                    Ok(Some((bindings, ops))) => {
                        self.advance_if_needed(next_op_index);
                        self.push_scoped_ops_with_bindings(bindings, ops);
                    }
                    Ok(None) => self.fail_eval(
                        RuntimeEvalError::PatternMismatch(scrutinee.to_string()),
                        output,
                    ),
                    Err(error) => self.fail_eval(error, output),
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
                    Err(error) => self.fail_eval(error, output),
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
                    Err(error) => self.fail_eval(error, output),
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
                Err(error) => self.fail_eval(error, output),
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
                Err(error) => self.fail_eval(error, output),
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
                        Err(error) => self.fail_eval(error, output),
                    },
                    Err(error) => self.fail_eval(error, output),
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
            FlowOp::Thread { name, body } => {
                self.advance_if_needed(next_op_index);
                output
                    .requests
                    .tasks
                    .push(flow_thread_task_spec(name.as_deref()));
                self.spawn_child_fiber(body);
            }
            FlowOp::Scope(ops) => {
                self.advance_if_needed(next_op_index);
                self.push_scoped_ops(ops);
            }
            FlowOp::LetScope {
                pattern,
                mut ops,
                value,
            } => {
                self.advance_if_needed(next_op_index);
                ops.insert(0, FlowOp::EnterScope);
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
                            self.fail_eval(error, output);
                            return;
                        }
                    },
                    None => RuntimeValue::Unit,
                };
                if self.break_nearest_loop(&value, output, pure_backend) {
                    self.advance_if_needed(next_op_index);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("break"), output);
                }
            }
            FlowOp::Continue => {
                if self.continue_nearest_loop(output, pure_backend) {
                    self.advance_if_needed(next_op_index);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                }
            }
            FlowOp::Goto(target) => self.goto(&target, output, pure_backend),
            FlowOp::GotoExpr(expr) => match self.evaluate_entity_target(&expr) {
                Ok(target) => self.goto(&target, output, pure_backend),
                Err(error) => self.fail_eval(error, output),
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
                            self.return_value(runtime_value_label(&value), output, pure_backend);
                        }
                        Err(error) => self.fail_eval(error, output),
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
                    Ok(Some(effect)) => self.emit_line_effect(effect, output, pure_backend),
                    Ok(None) => {}
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                }
                if !self.apply_control_effects(output, pure_backend) {
                    self.advance_if_needed(next_op_index);
                }
            }
            FlowOp::RegisterCleanup { key, effect } => {
                self.register_scope_cleanup(key, effect);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::CancelCleanup { key } => {
                self.cancel_scope_cleanup(&key);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::EnterScope => {
                self.push_scope_frame();
                self.advance_if_needed(next_op_index);
            }
            FlowOp::ExitScope => {
                self.pop_scope_frame(output, pure_backend);
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
                self.fiber.status = FlowFiberStatus::Waiting(*state);
            }
            FlowOp::ExitScopeBind { pattern, expr } => {
                let value = match self.evaluate_expr_with_backend(&expr, pure_backend) {
                    Ok(value) => value,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                self.pop_scope_frame(output, pure_backend);
                self.bind_value(&pattern, &value, output);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::Noop => {
                self.advance_if_needed(next_op_index);
            }
        }
    }

    fn evaluate_host_call_arguments(
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
                RuntimeHostArgumentTemplate::Positional(_) => {
                    positional.push(crate::value::RuntimePayload::from(value));
                }
                RuntimeHostArgumentTemplate::Named(argument) => named.push(NamedHostArg {
                    name: argument.name.clone(),
                    value: crate::value::RuntimePayload::from(value),
                }),
                RuntimeHostArgumentTemplate::Spread(_) => {
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
        value: &RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        match self.try_bind_pattern(pattern, value) {
            Ok(true) => {}
            Ok(false) => self.fail_eval(
                RuntimeEvalError::PatternMismatch(runtime_value_label(value)),
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

    fn next_host_call_id(&mut self, public_id: &str) -> RuntimeHostCallId {
        let sequence = self.next_host_call_sequence;
        self.next_host_call_sequence = self.next_host_call_sequence.saturating_add(1);
        RuntimeHostCallId(if sequence == 0 {
            public_id.to_owned()
        } else {
            format!("{public_id}.{sequence}")
        })
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

    pub(super) fn push_scope_frame(&mut self) {
        self.fiber.env.push_scope();
        self.fiber.control_stack.push(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope {
                cleanups: Vec::new(),
            },
        });
    }

    pub(super) fn register_scope_cleanup(
        &mut self,
        key: impl Into<String>,
        effect: LineEffectRequest,
    ) {
        let cleanup = FlowScopeCleanup::new(key, effect);
        if let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope { cleanups },
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
            if let FlowControlStackEntryKind::Scope { cleanups } = &mut entry.kind {
                cleanups.retain(|cleanup| cleanup.key != key);
            }
        }
    }

    pub(super) fn push_scoped_ops(&mut self, ops: Vec<FlowOp>) {
        self.push_owned_scoped_ops(ops, None);
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
        self.push_for_item(pattern, iterator, evidence, body, &item, output);
    }

    fn push_for_item(
        &mut self,
        pattern: RuntimePattern,
        iterator: RuntimeIterator,
        evidence: RuntimeIteratorEvidence,
        body: &Arc<[FlowOp]>,
        item: &RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        self.fiber
            .env
            .push_scope_with_capacity(pattern_binding_capacity(&pattern));
        self.fiber.control_stack.push(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope {
                cleanups: Vec::new(),
            },
        });
        match self.try_bind_pattern(&pattern, item) {
            Ok(true) => {}
            Ok(false) => {
                self.fail_eval(
                    RuntimeEvalError::PatternMismatch(runtime_value_label(item)),
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
        self.push_borrowed_ops_with_exit(body.as_ref(), Some(tail));
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
            (**state).clone(),
            Vec::new(),
            pure_backend,
        )?;
        if let Some(updated_receiver) = outcome.updated_receiver {
            **state = updated_receiver;
        }
        let RuntimeValue::Variant { name, payload, .. } = outcome.value else {
            return Ok(None);
        };
        if name == "None" {
            return Ok(None);
        }
        if name == "Some" {
            return Ok(payload.map(|value| *value));
        }
        Ok(None)
    }

    fn push_owned_scoped_ops(&mut self, ops: Vec<FlowOp>, prefix: Option<FlowOp>) {
        if ops.is_empty() && prefix.is_none() {
            return;
        }
        self.fiber
            .pending_ops
            .reserve(ops.len() + usize::from(prefix.is_some()) + 2);
        self.fiber.pending_ops.push_front(FlowOp::ExitScope);
        for op in ops.into_iter().rev() {
            self.fiber.pending_ops.push_front(op);
        }
        if let Some(prefix) = prefix {
            self.fiber.pending_ops.push_front(prefix);
        }
        self.fiber.pending_ops.push_front(FlowOp::EnterScope);
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
        self.fiber.pending_ops.push_front(FlowOp::ExitScope);
        for op in ops.iter().rev().cloned() {
            self.fiber.pending_ops.push_front(op);
        }
        if let Some(prefix) = prefix {
            self.fiber.pending_ops.push_front(prefix);
        }
        self.fiber.pending_ops.push_front(FlowOp::EnterScope);
    }

    fn push_borrowed_ops_with_exit(&mut self, ops: &[FlowOp], tail: Option<FlowOp>) {
        self.fiber
            .pending_ops
            .reserve(ops.len() + usize::from(tail.is_some()) + 1);
        if let Some(tail) = tail {
            self.fiber.pending_ops.push_front(tail);
        }
        self.fiber.pending_ops.push_front(FlowOp::ExitScope);
        for op in ops.iter().rev().cloned() {
            self.fiber.pending_ops.push_front(op);
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
            kind: FlowControlStackEntryKind::Scope { cleanups },
        }) = self.fiber.control_stack.pop()
        else {
            return;
        };
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
        match self.fiber.control_stack.pop() {
            Some(FlowControlStackEntry {
                kind:
                    kind @ (FlowControlStackEntryKind::Loop { .. }
                    | FlowControlStackEntryKind::While { .. }
                    | FlowControlStackEntryKind::WhileLet { .. }),
            }) => Some(kind),
            _ => None,
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
        value: &RuntimeValue,
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
            } => self.bind_value(&pattern, value, output),
            FlowControlStackEntryKind::Loop { result: None, .. } => {}
            FlowControlStackEntryKind::While { .. }
            | FlowControlStackEntryKind::WhileLet { .. } => {
                if *value != RuntimeValue::Unit {
                    self.fail_eval(RuntimeEvalError::BreakValueOutsideValueLoop, output);
                }
            }
            FlowControlStackEntryKind::Scope { .. } => return false,
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

fn flow_thread_task_spec(name: Option<&str>) -> TaskSpec {
    let label = name.unwrap_or("anonymous");
    let id = TaskId(format!("flow.thread.{label}"));
    TaskSpec::new(
        id,
        TaskKey(format!("flow.thread.{label}")),
        TaskClass::Cpu,
        TaskPriority(0),
        CancelScopeId("flow".to_owned()),
        TaskPolicy::AlwaysStart,
        HostTaskRequest::custom("flow_thread", "run_child", [label.into()]),
    )
}
