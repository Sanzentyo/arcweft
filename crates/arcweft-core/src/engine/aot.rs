use super::{
    Engine, FlowFiberStatus, RuntimeStepInput, RuntimeStepOptions, RuntimeStepOutput,
    RuntimeStepResult, RuntimeStepStats,
};
use crate::aot::{AotLinearOp, AotProgram};
use crate::pure::RuntimeCallBackend;

impl Engine {
    pub(crate) fn step_prechecked_aot_linear_with_pure_backend(
        &mut self,
        program: &AotProgram,
        input: RuntimeStepInput,
        root_bindings: &[crate::value::RuntimeBinding],
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> (RuntimeStepResult, usize) {
        let mut output = RuntimeStepOutput::default();
        let mut executed_ops = 0;
        let mut aot_fast_path_ops = 0;
        let pure_stats_before = pure_backend.stats();
        let pending_ops_before = self.fiber.pending_ops.len();
        self.fiber.env.bind_all_root_ref(root_bindings);
        self.fiber.env.bind_all_root(input.bindings);
        let runtime_input = RuntimeStepInput::default();

        while executed_ops < options.budget.max_ops && self.can_attempt_runtime_op() {
            if !self.fiber.pending_ops.is_empty() {
                self.step_runtime_op(&runtime_input, &[], &mut output, pure_backend);
                executed_ops += 1;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            }
            let Some(cursor) = self.fiber.cursor.as_ref() else {
                self.finish(&mut output, pure_backend);
                executed_ops += 1;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            };
            let Some(flow) = program.flow_block(cursor.flow_index) else {
                self.fail_aot_linear_precondition(
                    "AOT linear cursor no longer references a flow",
                    &mut output,
                );
                break;
            };
            let Some(op) = flow.linear_op(cursor.op_index) else {
                if cursor.op_index >= flow.ops {
                    self.finish(&mut output, pure_backend);
                } else {
                    self.step_runtime_op(&runtime_input, &[], &mut output, pure_backend);
                }
                executed_ops += 1;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            };
            let next_op_index = cursor.op_index + 1;
            self.step_aot_linear_op(op, next_op_index, &mut output, pure_backend);
            executed_ops += 1;
            aot_fast_path_ops += 1;
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
            need_states_in: 0,
            source_events_in: 0,
            root_events_in: 0,
            root_transitions: 0,
            root_commands: 0,
            root_events_deferred: 0,
            source_events_emitted: 0,
            stream_events_emitted: 0,
            line_effects: output.effects.line.len(),
            audio_commands: output.requests.audio.len(),
            diagnostics: output.diagnostics.len(),
        };
        (self.step_result(output, options, stats), aot_fast_path_ops)
    }

    fn fail_aot_linear_precondition(&mut self, message: &str, output: &mut RuntimeStepOutput) {
        output
            .diagnostics
            .push(super::RuntimeDiagnostic::new(message.to_owned()));
        self.fiber.status = FlowFiberStatus::Failed(message.to_owned());
    }

    pub(crate) fn can_start_aot_linear_step(
        &self,
        program: &AotProgram,
        input: &RuntimeStepInput,
    ) -> bool {
        input.task_events.is_empty()
            && input.source_events.is_empty()
            && input.root_events.is_empty()
            && self.root.is_none()
            && self.plan.source_plans.is_empty()
            && self.plan.stream_plans.is_empty()
            && self.fiber.pending_ops.is_empty()
            && self.fiber.control_stack.is_empty()
            && matches!(self.fiber.status, FlowFiberStatus::Running)
            && self.fiber.cursor.as_ref().is_some_and(|cursor| {
                program
                    .flow_block(cursor.flow_index)
                    .and_then(|flow| flow.linear_op(cursor.op_index))
                    .is_some()
            })
    }

    fn step_aot_linear_op(
        &mut self,
        op: &AotLinearOp,
        next_op_index: usize,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        match op {
            AotLinearOp::Bind(bindings) => {
                self.fiber.env.bind_all_ref(bindings);
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::Let { pattern, expr } => {
                self.evaluate_let_with_backend(pattern, expr, output, pure_backend);
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::Return(value) => self.return_value(value.clone(), output, pure_backend),
            AotLinearOp::ReturnExpr(expr) => {
                match self.evaluate_expr_with_backend(expr, pure_backend) {
                    Ok(value) => {
                        self.return_value(super::runtime_value_label(&value), output, pure_backend);
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            AotLinearOp::Effect(effect) => {
                self.emit_line_effect(effect.clone(), output, pure_backend);
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::RegisterCleanup { key, effect } => {
                self.register_scope_cleanup(key.clone(), effect.clone());
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::CancelCleanup { key } => {
                self.cancel_scope_cleanup(key);
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::EnterScope => {
                self.push_scope_frame();
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::ExitScope => {
                self.pop_scope_frame(output, pure_backend);
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::ExitScopeBind { pattern, expr } => {
                match self.evaluate_expr_with_backend(expr, pure_backend) {
                    Ok(value) => {
                        self.pop_scope_frame(output, pure_backend);
                        self.bind_value(pattern, &value, output);
                        self.advance_aot_linear_cursor(next_op_index);
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            AotLinearOp::Noop => self.advance_aot_linear_cursor(next_op_index),
        }
    }

    fn advance_aot_linear_cursor(&mut self, next_op_index: usize) {
        if let Some(cursor) = self.fiber.cursor.as_mut() {
            cursor.op_index = next_op_index;
        }
    }
}
