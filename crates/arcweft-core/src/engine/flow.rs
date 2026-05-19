use super::suspend::await_task_spec;
use super::{
    AwaitState, ChoiceState, Engine, FlowCursor, FlowEvent, FlowFiberStatus, FlowOp, FlowRuntimeId,
    FrameInput, FrameOutput, RuntimeBinding, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr,
    RuntimeFrame, RuntimeFrameKind, RuntimePattern, RuntimeValue, expr_runtime_label,
    run_line_task_group_for_input, runtime_value_label,
};

impl Engine {
    // Keep the opcode dispatcher contiguous while the Phase 1 runtime surface is
    // still changing; extracting each arm now would obscure grammar coverage.
    #[allow(clippy::too_many_lines)]
    pub(super) fn step_flow(&mut self, input: &FrameInput, output: &mut FrameOutput) {
        let (op, next) = if let Some(op) = self.fiber.pending_ops.pop_front() {
            (op, None)
        } else {
            let Some(cursor) = self.fiber.cursor.clone() else {
                return;
            };
            let Some(op) = self
                .plan
                .flows
                .iter()
                .find(|flow| flow.id == cursor.flow)
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
                self.evaluate_let(&pattern, &expr, output);
                self.advance_if_needed(next);
            }
            FlowOp::LetElse {
                pattern,
                expr,
                else_ops,
            } => {
                match self.evaluate_expr(&expr).and_then(|value| {
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
            FlowOp::Await { target, pending } => {
                output.flow_events.push(FlowEvent::AwaitStarted {
                    need: target.need.clone(),
                    task: target.task.clone(),
                });
                output.line_effects.extend(pending);
                output.task_requests.push(await_task_spec(&target));
                self.fiber.status = FlowFiberStatus::Waiting(AwaitState {
                    target,
                    resume: next
                        .or_else(|| self.fiber.cursor.clone())
                        .unwrap_or_default(),
                });
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => match self.evaluate_bool(&condition) {
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
            } => match self.evaluate_if_let(&pattern, &expr, guard.as_ref()) {
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
            FlowOp::Match { scrutinee, arms } => match self.evaluate_match(&scrutinee, &arms) {
                Ok(Some((bindings, ops))) => {
                    self.advance_if_needed(next);
                    self.push_scoped_ops_with_bindings(bindings, ops);
                }
                Ok(None) => self.fail_eval(
                    RuntimeEvalError::PatternMismatch(expr_runtime_label(&scrutinee)),
                    output,
                ),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Loop { body } => {
                self.advance_if_needed(next);
                self.fiber.frames.push(RuntimeFrame {
                    kind: RuntimeFrameKind::Loop {
                        body: body.clone(),
                        result: None,
                    },
                });
                self.push_loop_iteration(body);
            }
            FlowOp::LetLoop { pattern, body } => {
                self.advance_if_needed(next);
                self.fiber.frames.push(RuntimeFrame {
                    kind: RuntimeFrameKind::Loop {
                        body: body.clone(),
                        result: Some(pattern),
                    },
                });
                self.push_loop_iteration(body);
            }
            FlowOp::LoopNext { body } => {
                self.push_loop_iteration(body);
            }
            FlowOp::While { condition, body } => match self.evaluate_bool(&condition) {
                Ok(true) => {
                    self.advance_if_needed(next);
                    self.fiber.frames.push(RuntimeFrame {
                        kind: RuntimeFrameKind::While {
                            condition: condition.clone(),
                            body: body.clone(),
                        },
                    });
                    self.push_while_iteration(condition, body);
                }
                Ok(false) => self.advance_if_needed(next),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::WhileNext { condition, body } => match self.evaluate_bool(&condition) {
                Ok(true) => {
                    self.push_while_iteration(condition, body);
                }
                Ok(false) => {
                    self.pop_loop_frame();
                }
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => match self.evaluate_if_let(&pattern, &expr, guard.as_ref()) {
                Ok(Some(bindings)) => {
                    self.advance_if_needed(next);
                    self.fiber.frames.push(RuntimeFrame {
                        kind: RuntimeFrameKind::WhileLet {
                            pattern: pattern.clone(),
                            expr: expr.clone(),
                            guard: guard.clone(),
                            body: body.clone(),
                        },
                    });
                    self.push_while_let_iteration(pattern, expr, guard, body, bindings);
                }
                Ok(None) => self.advance_if_needed(next),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::WhileLetNext {
                pattern,
                expr,
                guard,
                body,
            } => match self.evaluate_if_let(&pattern, &expr, guard.as_ref()) {
                Ok(Some(bindings)) => {
                    self.push_while_let_iteration(pattern, expr, guard, body, bindings);
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
                match self.evaluate_expr(&source) {
                    Ok(RuntimeValue::BracketSeq(items)) => {
                        let mut ops = Vec::new();
                        for item in items {
                            ops.push(FlowOp::EnterScope);
                            ops.push(FlowOp::Let {
                                pattern: pattern.clone(),
                                expr: RuntimeExpr::Value(item),
                            });
                            ops.extend(body.clone());
                            ops.push(FlowOp::ExitScope);
                        }
                        self.push_ops(ops);
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
                    Some(expr) => match self.evaluate_expr(&expr) {
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
            FlowOp::Return(value) => self.return_value(value, output),
            FlowOp::ReturnExpr(expr) => match self.evaluate_expr(&expr) {
                Ok(value) => self.return_value(runtime_value_label(&value), output),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Effect(effect) => {
                output.line_effects.push(effect);
                if !self.apply_control_effects(output) {
                    self.advance_if_needed(next);
                }
            }
            FlowOp::EnterScope => {
                self.fiber.env.push_scope();
                self.fiber.frames.push(RuntimeFrame {
                    kind: RuntimeFrameKind::Scope,
                });
                self.advance_if_needed(next);
            }
            FlowOp::ExitScope => {
                self.pop_scope_frame();
                self.advance_if_needed(next);
            }
            FlowOp::ExitScopeBind { pattern, expr } => {
                let value = match self.evaluate_expr(&expr) {
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
        output: &mut FrameOutput,
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
        self.push_ops(Self::scoped_ops(ops));
    }

    pub(super) fn push_scoped_ops_with_bindings(
        &mut self,
        bindings: Vec<RuntimeBinding>,
        mut ops: Vec<FlowOp>,
    ) {
        if bindings.is_empty() && ops.is_empty() {
            return;
        }
        ops.insert(0, FlowOp::Bind(bindings));
        self.push_scoped_ops(ops);
    }

    pub(super) fn push_loop_iteration(&mut self, body: Vec<FlowOp>) {
        let mut ops = Self::scoped_ops(body.clone());
        ops.push(FlowOp::LoopNext { body });
        self.push_ops(ops);
    }

    pub(super) fn push_while_iteration(&mut self, condition: RuntimeExpr, body: Vec<FlowOp>) {
        let mut ops = Self::scoped_ops(body.clone());
        ops.push(FlowOp::WhileNext { condition, body });
        self.push_ops(ops);
    }

    pub(super) fn push_while_let_iteration(
        &mut self,
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
        bindings: Vec<RuntimeBinding>,
    ) {
        let mut scoped = body.clone();
        scoped.insert(0, FlowOp::Bind(bindings));
        let mut ops = Self::scoped_ops(scoped);
        ops.push(FlowOp::WhileLetNext {
            pattern,
            expr,
            guard,
            body,
        });
        self.push_ops(ops);
    }

    pub(super) fn pop_scope_frame(&mut self) {
        if matches!(
            self.fiber.frames.last(),
            Some(RuntimeFrame {
                kind: RuntimeFrameKind::Scope
            })
        ) {
            self.fiber.frames.pop();
            self.fiber.env.pop_scope();
        }
    }

    pub(super) fn pop_scope_frames_until_loop(&mut self) {
        while matches!(
            self.fiber.frames.last(),
            Some(RuntimeFrame {
                kind: RuntimeFrameKind::Scope
            })
        ) {
            self.pop_scope_frame();
        }
    }

    pub(super) fn pop_loop_frame(&mut self) -> Option<RuntimeFrameKind> {
        self.pop_scope_frames_until_loop();
        match self.fiber.frames.pop() {
            Some(RuntimeFrame {
                kind:
                    kind @ (RuntimeFrameKind::Loop { .. }
                    | RuntimeFrameKind::While { .. }
                    | RuntimeFrameKind::WhileLet { .. }),
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
        output: &mut FrameOutput,
    ) -> bool {
        self.discard_pending_until_loop_next();
        let Some(kind) = self.pop_loop_frame() else {
            return false;
        };
        match kind {
            RuntimeFrameKind::Loop {
                result: Some(pattern),
                ..
            } => self.bind_value(&pattern, value, output),
            RuntimeFrameKind::Loop { result: None, .. } => {}
            RuntimeFrameKind::While { .. } | RuntimeFrameKind::WhileLet { .. } => {
                if *value != RuntimeValue::Unit {
                    self.fail_eval(RuntimeEvalError::BreakValueOutsideValueLoop, output);
                }
            }
            RuntimeFrameKind::Scope => return false,
        }
        true
    }

    pub(super) fn continue_nearest_loop(&mut self, output: &mut FrameOutput) -> bool {
        self.pop_scope_frames_until_loop();
        self.discard_pending_until_loop_next();
        let Some(kind) = self.fiber.frames.last().map(|frame| frame.kind.clone()) else {
            return false;
        };
        match kind {
            RuntimeFrameKind::Loop { body, .. } => self.push_loop_iteration(body),
            RuntimeFrameKind::While { condition, body } => {
                self.push_ops(vec![FlowOp::WhileNext { condition, body }]);
            }
            RuntimeFrameKind::WhileLet {
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
            RuntimeFrameKind::Scope => {
                self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                return false;
            }
        }
        true
    }
}
