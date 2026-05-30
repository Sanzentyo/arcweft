use super::{
    AwaitState, ChoiceState, Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowCursor,
    FlowEvent, FlowFiberStatus, FlowOp, FlowRuntimeId, RuntimeBinding, RuntimeDiagnostic,
    RuntimeEvalError, RuntimeExpr, RuntimePattern, RuntimeStepInput, RuntimeStepOutput,
    RuntimeValue, expr_runtime_label, run_line_task_group_for_input, runtime_value_label,
};
use crate::pure::RuntimePureCallBackend;
use crate::task::{
    CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskPolicy, TaskPriority, TaskSpec,
};
use std::sync::Arc;

impl Engine {
    // Keep the opcode dispatcher contiguous while the Phase 1 runtime surface is
    // still changing; extracting each arm now would obscure grammar coverage.
    #[allow(clippy::too_many_lines)]
    pub(super) fn step_flow(
        &mut self,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) {
        let (op, next) = if let Some(op) = self.fiber.pending_ops.pop_front() {
            (op, None)
        } else {
            let Some(cursor) = self.fiber.cursor.clone() else {
                return;
            };
            let Some(op) = self
                .flow_at_cursor(&cursor)
                .and_then(|flow| flow.ops.get(cursor.op_index))
                .cloned()
            else {
                self.finish(output);
                return;
            };
            let next = cursor.advanced();
            (op, Some(next))
        };
        match op {
            FlowOp::Bind(bindings) => {
                self.fiber.env.bind_all(bindings);
                self.advance_if_needed(next);
            }
            FlowOp::Let { pattern, expr } => {
                self.evaluate_let_with_backend(&pattern, &expr, output, pure_backend);
                self.advance_if_needed(next);
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
                    Ok((true, _)) => self.advance_if_needed(next),
                    Ok((false, value)) => {
                        self.advance_if_needed(next);
                        self.push_ops(else_ops);
                        output.diagnostics.push(RuntimeDiagnostic {
                            message: format!(
                                "let-else pattern did not match {}",
                                runtime_value_label(&value)
                            ),
                        });
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::Dialogue { line, task_group } => {
                output.flow_events.push(FlowEvent::DialogueLine { line });
                let Some(group) = self.plan.line_task_groups.get(task_group) else {
                    self.fiber.status =
                        FlowFiberStatus::Failed(format!("missing line task group {task_group}"));
                    return;
                };
                output.merge(run_line_task_group_for_input(group, input));
                if !self.apply_control_effects(output) {
                    self.advance_if_needed(next);
                }
            }
            FlowOp::Choice { id, options } => {
                output
                    .flow_events
                    .push(FlowEvent::ChoicePresented { id: id.clone() });
                self.fiber.status = FlowFiberStatus::Choice(ChoiceState {
                    id,
                    options,
                    resume: next
                        .or_else(|| self.fiber.cursor.clone())
                        .unwrap_or_default(),
                });
            }
            FlowOp::Await {
                binding,
                target,
                pending,
            } => {
                output.flow_events.push(FlowEvent::AwaitStarted {
                    need: target.need.clone(),
                    task: target.task.clone(),
                });
                output.effects.line.extend(pending);
                let Some(task) = self.await_task_spec(&target, output, pure_backend) else {
                    return;
                };
                output.requests.tasks.push(task);
                self.fiber.status = FlowFiberStatus::Waiting(AwaitState {
                    binding,
                    target,
                    resume: next
                        .or_else(|| self.fiber.cursor.clone())
                        .unwrap_or_default(),
                });
            }
            FlowOp::AwaitMany {
                binding,
                target,
                pending,
            } => {
                output.effects.line.extend(pending);
                self.start_await_many_state(
                    binding,
                    target,
                    next.or_else(|| self.fiber.cursor.clone())
                        .unwrap_or_default(),
                    output,
                    pure_backend,
                );
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => match self.evaluate_bool_with_backend(&condition, pure_backend) {
                Ok(true) => {
                    self.advance_if_needed(next);
                    self.push_scoped_ops(then_ops);
                }
                Ok(false) => {
                    self.advance_if_needed(next);
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
                    self.advance_if_needed(next);
                    self.push_scoped_ops_with_bindings(bindings, then_ops);
                }
                Ok(None) => {
                    self.advance_if_needed(next);
                    self.push_scoped_ops(else_ops);
                }
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Match { scrutinee, arms } => {
                match self.evaluate_match_with_backend(&scrutinee, arms, pure_backend) {
                    Ok(Some((bindings, ops))) => {
                        self.advance_if_needed(next);
                        self.push_scoped_ops_with_bindings(bindings, ops);
                    }
                    Ok(None) => self.fail_eval(
                        RuntimeEvalError::PatternMismatch(expr_runtime_label(&scrutinee)),
                        output,
                    ),
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::Loop { body } => {
                self.advance_if_needed(next);
                let body = Arc::from(body);
                self.fiber.control_stack.push(FlowControlStackEntry {
                    kind: FlowControlStackEntryKind::Loop {
                        body: Arc::clone(&body),
                        result: None,
                    },
                });
                self.push_loop_iteration(&body);
            }
            FlowOp::LetLoop { pattern, body } => {
                self.advance_if_needed(next);
                let body = Arc::from(body);
                self.fiber.control_stack.push(FlowControlStackEntry {
                    kind: FlowControlStackEntryKind::Loop {
                        body: Arc::clone(&body),
                        result: Some(pattern),
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
                        self.advance_if_needed(next);
                        let body = Arc::from(body);
                        self.fiber.control_stack.push(FlowControlStackEntry {
                            kind: FlowControlStackEntryKind::While {
                                condition: condition.clone(),
                                body: Arc::clone(&body),
                            },
                        });
                        self.push_while_iteration(condition, &body);
                    }
                    Ok(false) => self.advance_if_needed(next),
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::WhileNext { condition, body } => {
                match self.evaluate_bool_with_backend(&condition, pure_backend) {
                    Ok(true) => {
                        self.push_while_iteration(condition, &body);
                    }
                    Ok(false) => {
                        self.pop_loop_frame();
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
                    self.advance_if_needed(next);
                    let body = Arc::from(body);
                    self.fiber.control_stack.push(FlowControlStackEntry {
                        kind: FlowControlStackEntryKind::WhileLet {
                            pattern: pattern.clone(),
                            expr: expr.clone(),
                            guard: guard.clone(),
                            body: Arc::clone(&body),
                        },
                    });
                    self.push_while_let_iteration(pattern, expr, guard, &body, bindings);
                }
                Ok(None) => self.advance_if_needed(next),
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
                    self.pop_loop_frame();
                }
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::For {
                pattern,
                source,
                body,
            } => {
                self.advance_if_needed(next);
                match self.evaluate_expr_with_backend(&source, pure_backend) {
                    Ok(RuntimeValue::BracketSeq(items)) => {
                        let body = Arc::from(body);
                        self.push_for_next(pattern, items.into(), 0, &body, output);
                    }
                    Ok(value) => {
                        self.fail_eval(
                            RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value)),
                            output,
                        );
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::ForNext {
                pattern,
                items,
                index,
                body,
            } => {
                self.push_for_next(pattern, items, index, &body, output);
            }
            FlowOp::Thread { name, body } => {
                self.advance_if_needed(next);
                output
                    .requests
                    .tasks
                    .push(flow_thread_task_spec(name.as_deref()));
                self.spawn_child_fiber(body);
            }
            FlowOp::Scope(ops) => {
                self.advance_if_needed(next);
                self.push_scoped_ops(ops);
            }
            FlowOp::LetScope {
                pattern,
                mut ops,
                value,
            } => {
                self.advance_if_needed(next);
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
                if self.break_nearest_loop(&value, output) {
                    self.advance_if_needed(next);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("break"), output);
                }
            }
            FlowOp::Continue => {
                if self.continue_nearest_loop(output) {
                    self.advance_if_needed(next);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                }
            }
            FlowOp::Goto(target) => self.goto(target, output),
            FlowOp::GotoExpr(expr) => match self.evaluate_entity_target(&expr) {
                Ok(target) => self.goto(FlowRuntimeId(target), output),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Return(value) => {
                if self.has_active_child_fibers() {
                    self.push_ops(vec![FlowOp::Return(value)]);
                    self.run_child_next = true;
                } else {
                    self.return_value(value, output);
                }
            }
            FlowOp::ReturnExpr(expr) => {
                if self.has_active_child_fibers() {
                    self.push_ops(vec![FlowOp::ReturnExpr(expr)]);
                    self.run_child_next = true;
                } else {
                    match self.evaluate_expr_with_backend(&expr, pure_backend) {
                        Ok(value) => self.return_value(runtime_value_label(&value), output),
                        Err(error) => self.fail_eval(error, output),
                    }
                }
            }
            FlowOp::Effect(effect) => {
                output.effects.line.push(effect);
                if !self.apply_control_effects(output) {
                    self.advance_if_needed(next);
                }
            }
            FlowOp::EnterScope => {
                self.fiber.env.push_scope();
                self.fiber.control_stack.push(FlowControlStackEntry {
                    kind: FlowControlStackEntryKind::Scope,
                });
                self.advance_if_needed(next);
            }
            FlowOp::ExitScope => {
                self.pop_scope_frame();
                self.advance_if_needed(next);
            }
            FlowOp::ExitScopeBind { pattern, expr } => {
                let value = match self.evaluate_expr_with_backend(&expr, pure_backend) {
                    Ok(value) => value,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                self.pop_scope_frame();
                self.bind_value(&pattern, &value, output);
                self.advance_if_needed(next);
            }
            FlowOp::Noop => {
                self.advance_if_needed(next);
            }
        }
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

    pub(super) fn advance_if_needed(&mut self, next: Option<FlowCursor>) {
        if let Some(next) = next {
            self.fiber.cursor = Some(next);
        }
    }

    pub(super) fn push_ops(&mut self, ops: Vec<FlowOp>) {
        self.fiber.pending_ops.reserve(ops.len());
        for op in ops.into_iter().rev() {
            self.fiber.pending_ops.push_front(op);
        }
    }

    pub(super) fn scoped_ops(mut ops: Vec<FlowOp>) -> Vec<FlowOp> {
        if ops.is_empty() {
            return Vec::new();
        }
        ops.insert(0, FlowOp::EnterScope);
        ops.push(FlowOp::ExitScope);
        ops
    }

    pub(super) fn push_scoped_ops(&mut self, ops: Vec<FlowOp>) {
        self.push_owned_scoped_ops(ops, None);
    }

    pub(super) fn push_scoped_ops_with_bindings(
        &mut self,
        bindings: Vec<RuntimeBinding>,
        ops: Vec<FlowOp>,
    ) {
        if bindings.is_empty() && ops.is_empty() {
            return;
        }
        let prefix = (!bindings.is_empty()).then_some(FlowOp::Bind(bindings));
        self.push_owned_scoped_ops(ops, prefix);
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
        bindings: Vec<RuntimeBinding>,
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
        items: Arc<[RuntimeValue]>,
        index: usize,
        body: &Arc<[FlowOp]>,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(item) = items.get(index).cloned() else {
            return;
        };
        self.fiber.env.push_scope();
        self.fiber.control_stack.push(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope,
        });
        match self.try_bind_pattern(&pattern, &item) {
            Ok(true) => {}
            Ok(false) => {
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
            items,
            index: index + 1,
            body: Arc::clone(body),
        };
        self.push_borrowed_ops_with_exit(body.as_ref(), Some(tail));
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

    pub(super) fn pop_scope_frame(&mut self) {
        if matches!(
            self.fiber.control_stack.last(),
            Some(FlowControlStackEntry {
                kind: FlowControlStackEntryKind::Scope
            })
        ) {
            self.fiber.control_stack.pop();
            self.fiber.env.pop_scope();
        }
    }

    pub(super) fn pop_scope_frames_until_loop(&mut self) {
        while matches!(
            self.fiber.control_stack.last(),
            Some(FlowControlStackEntry {
                kind: FlowControlStackEntryKind::Scope
            })
        ) {
            self.pop_scope_frame();
        }
    }

    pub(super) fn pop_loop_frame(&mut self) -> Option<FlowControlStackEntryKind> {
        self.pop_scope_frames_until_loop();
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
    ) -> bool {
        self.discard_pending_until_loop_next();
        let Some(kind) = self.pop_loop_frame() else {
            return false;
        };
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
            FlowControlStackEntryKind::Scope => return false,
        }
        true
    }

    pub(super) fn continue_nearest_loop(&mut self, output: &mut RuntimeStepOutput) -> bool {
        self.pop_scope_frames_until_loop();
        self.discard_pending_until_loop_next();
        let Some(kind) = self
            .fiber
            .control_stack
            .last()
            .map(|frame| frame.kind.clone())
        else {
            return false;
        };
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
                    guard,
                    body,
                }]);
            }
            FlowControlStackEntryKind::Scope => {
                self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                return false;
            }
        }
        true
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
