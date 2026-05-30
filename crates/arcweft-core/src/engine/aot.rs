use super::{
    Engine, FlowFiberStatus, FlowOp, RuntimeStepInput, RuntimeStepOptions, RuntimeStepOutput,
    RuntimeStepResult, RuntimeStepStats,
};
use crate::aot::{AotDispatchShape, AotProgram, aot_linear_supported_op};
use crate::pure::RuntimePureCallBackend;

impl Engine {
    pub(crate) fn step_prechecked_aot_linear_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[crate::value::RuntimeBinding],
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> RuntimeStepResult {
        let mut output = RuntimeStepOutput::default();
        let mut executed_ops = 0;
        let pure_stats_before = pure_backend.stats();
        let pending_ops_before = self.fiber.pending_ops.len();
        self.fiber.env.bind_all_root_ref(root_bindings);
        self.fiber.env.bind_all_root(input.bindings);

        while executed_ops < options.budget.max_ops && self.can_attempt_runtime_op() {
            let Some(cursor) = self.fiber.cursor.as_ref() else {
                self.finish(&mut output);
                executed_ops += 1;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            };
            let Some(flow) = self.flow_at_cursor(cursor) else {
                self.fail_aot_linear_precondition(
                    "AOT linear cursor no longer references a flow",
                    &mut output,
                );
                break;
            };
            let Some(op) = flow.ops.get(cursor.op_index).cloned() else {
                self.finish(&mut output);
                executed_ops += 1;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            };
            if !aot_linear_supported_op(&op) {
                self.fail_aot_linear_precondition(
                    "AOT linear program contains an unsupported op",
                    &mut output,
                );
                break;
            }
            let next_op_index = cursor.op_index + 1;
            self.step_aot_linear_op(op, next_op_index, &mut output, pure_backend);
            executed_ops += 1;
            if self.should_return_to_host(options.mode, &output, executed_ops) {
                break;
            }
        }

        self.record_observations(&output.effects.line);
        let stats = RuntimeStepStats {
            executed_ops,
            pending_ops_before,
            pending_ops_after: self.fiber.pending_ops.len(),
            child_fibers: self.child_fiber_count(),
            pure: pure_backend.stats().saturating_delta(pure_stats_before),
            task_events_in: 0,
            source_events_in: 0,
            source_events_emitted: 0,
            stream_events_emitted: 0,
            line_effects: output.effects.line.len(),
            diagnostics: output.diagnostics.len(),
        };
        self.step_result(output, options, stats)
    }

    fn fail_aot_linear_precondition(&mut self, message: &str, output: &mut RuntimeStepOutput) {
        output.diagnostics.push(super::RuntimeDiagnostic {
            message: message.to_owned(),
        });
        self.fiber.status = FlowFiberStatus::Failed(message.to_owned());
    }

    pub(crate) fn can_start_aot_linear_step(
        &self,
        program: &AotProgram,
        input: &RuntimeStepInput,
    ) -> bool {
        input.task_events.is_empty()
            && input.source_events.is_empty()
            && self.plan.source_plans.is_empty()
            && self.plan.stream_plans.is_empty()
            && self.fiber.pending_ops.is_empty()
            && self.fiber.control_stack.is_empty()
            && matches!(self.fiber.status, FlowFiberStatus::Running)
            && self
                .fiber
                .cursor
                .as_ref()
                .and_then(|cursor| {
                    program
                        .flow_block(cursor.flow_index)
                        .filter(|flow| flow.id == cursor.flow)
                })
                .is_some_and(|flow| flow.dispatch == AotDispatchShape::Linear)
    }

    fn step_aot_linear_op(
        &mut self,
        op: FlowOp,
        next_op_index: usize,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) {
        match op {
            FlowOp::Bind(bindings) => {
                self.fiber.env.bind_all(bindings);
                self.advance_aot_linear_cursor(next_op_index);
            }
            FlowOp::Let { pattern, expr } => {
                self.evaluate_let_with_backend(&pattern, &expr, output, pure_backend);
                self.advance_aot_linear_cursor(next_op_index);
            }
            FlowOp::Return(value) => self.return_value(value, output),
            FlowOp::ReturnExpr(expr) => {
                match self.evaluate_expr_with_backend(&expr, pure_backend) {
                    Ok(value) => self.return_value(super::runtime_value_label(&value), output),
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::Effect(effect) => {
                output.effects.line.push(effect);
                self.advance_aot_linear_cursor(next_op_index);
            }
            FlowOp::EnterScope => {
                self.fiber.env.push_scope();
                self.fiber.control_stack.push(super::FlowControlStackEntry {
                    kind: super::FlowControlStackEntryKind::Scope,
                });
                self.advance_aot_linear_cursor(next_op_index);
            }
            FlowOp::ExitScope => {
                self.pop_scope_frame();
                self.advance_aot_linear_cursor(next_op_index);
            }
            FlowOp::ExitScopeBind { pattern, expr } => {
                match self.evaluate_expr_with_backend(&expr, pure_backend) {
                    Ok(value) => {
                        self.pop_scope_frame();
                        self.bind_value(&pattern, &value, output);
                        self.advance_aot_linear_cursor(next_op_index);
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::Noop => self.advance_aot_linear_cursor(next_op_index),
            FlowOp::LetElse { .. }
            | FlowOp::Dialogue { .. }
            | FlowOp::Choice { .. }
            | FlowOp::Await { .. }
            | FlowOp::AwaitMany { .. }
            | FlowOp::If { .. }
            | FlowOp::IfLet { .. }
            | FlowOp::Match { .. }
            | FlowOp::Loop { .. }
            | FlowOp::LetLoop { .. }
            | FlowOp::LoopNext { .. }
            | FlowOp::While { .. }
            | FlowOp::WhileNext { .. }
            | FlowOp::WhileLet { .. }
            | FlowOp::WhileLetNext { .. }
            | FlowOp::For { .. }
            | FlowOp::ForNext { .. }
            | FlowOp::Thread { .. }
            | FlowOp::Scope(_)
            | FlowOp::LetScope { .. }
            | FlowOp::Break(_)
            | FlowOp::Continue
            | FlowOp::Goto(_)
            | FlowOp::GotoExpr(_) => {
                self.fiber.status = FlowFiberStatus::Failed(
                    "unsupported AOT linear operation reached executor".to_owned(),
                );
                output.diagnostics.push(super::RuntimeDiagnostic {
                    message: "unsupported AOT linear operation reached executor".to_owned(),
                });
            }
        }
    }

    fn advance_aot_linear_cursor(&mut self, next_op_index: usize) {
        if let Some(cursor) = self.fiber.cursor.as_mut() {
            cursor.op_index = next_op_index;
        }
    }
}
