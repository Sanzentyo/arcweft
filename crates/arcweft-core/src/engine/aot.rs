use super::{
    Engine, FlowFiberStatus, RuntimeStepInput, RuntimeStepOptions, RuntimeStepOutput,
    RuntimeStepResult, RuntimeStepStats,
};
use crate::aot::{AotLinearOp, AotProgram};
use crate::pure::RuntimeCallBackend;

#[cfg(test)]
mod tests;

impl Engine {
    pub(crate) fn step_prechecked_aot_linear_with_pure_backend(
        &mut self,
        program: &AotProgram,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> (RuntimeStepResult, usize) {
        let mut output = RuntimeStepOutput::default();
        self.task_request_quota_remaining = options.max_new_task_requests;
        let mut executed_ops = 0;
        let mut aot_fast_path_ops = 0;
        let pure_stats_before = pure_backend.stats();
        let pending_ops_before = self.pending_ops_len();
        let mut runtime_input = RuntimeStepInput::default();

        while executed_ops < options.budget.max_ops && self.can_attempt_runtime_op() {
            if let Some(consumed) = self.try_step_numeric_map_span(
                options.budget.max_ops - executed_ops,
                options.mode,
                &mut output,
                pure_backend,
            ) {
                executed_ops += consumed;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            }
            // Fallback operations may spawn/goto while a linear prefix is
            // active. Executor work always returns to the existing native
            // scheduler, including detached work, join-deferred returns and an
            // active function whose exhausted pending body must reject fallthrough.
            if self.has_executor_work()
                || self.has_active_project_call()
                || self.fiber.frame.pending_ops.len() != 1
                || self.fiber.frame.function_root_position().is_none()
            {
                let (_, control_ops) = self.with_scalar_operation_budget(
                    options.mode,
                    options.budget.max_ops - executed_ops - 1,
                    |engine| {
                        engine.step_runtime_op(&mut runtime_input, &[], &mut output, pure_backend)
                    },
                );
                executed_ops += 1 + control_ops;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            }
            let Some(cursor) = self.fiber.frame.function_root_position() else {
                self.finish(&mut output, pure_backend);
                executed_ops += 1;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            };
            let Some(flow) = (match cursor.owner() {
                crate::plan::code::RuntimeCodeOwner::Function(site) if cursor.is_root() => {
                    program.flow_block(site)
                }
                _ => None,
            }) else {
                self.fail_aot_linear_precondition(
                    "AOT linear cursor no longer references a flow",
                    &mut output,
                );
                break;
            };
            let position = cursor
                .operation_index()
                .expect("AOT block owns operation code");
            let Some(op) = flow.linear_op(position) else {
                if position >= flow.ops {
                    self.finish(&mut output, pure_backend);
                } else {
                    let (_, control_ops) = self.with_scalar_operation_budget(
                        options.mode,
                        options.budget.max_ops - executed_ops - 1,
                        |engine| {
                            engine.step_runtime_op(
                                &mut runtime_input,
                                &[],
                                &mut output,
                                pure_backend,
                            )
                        },
                    );
                    executed_ops += control_ops;
                }
                executed_ops += 1;
                if self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            };
            let next_op_index = position + 1;
            let (_, control_ops) = self.with_scalar_operation_budget(
                options.mode,
                options.budget.max_ops - executed_ops - 1,
                |engine| engine.step_aot_linear_op(op, next_op_index, &mut output, pure_backend),
            );
            executed_ops += 1 + control_ops;
            aot_fast_path_ops += 1;
            if self.should_return_to_host(options.mode, &output, executed_ops) {
                break;
            }
        }

        self.record_observations(&output.effects.line);
        let stats = self.aot_linear_step_stats(
            executed_ops,
            pending_ops_before,
            pure_backend.stats().saturating_delta(pure_stats_before),
            &output,
        );
        (self.step_result(output, options, stats), aot_fast_path_ops)
    }

    fn aot_linear_step_stats(
        &self,
        executed_ops: usize,
        pending_ops_before: usize,
        pure: crate::step::RuntimePureCallStats,
        output: &RuntimeStepOutput,
    ) -> RuntimeStepStats {
        RuntimeStepStats {
            executed_ops,
            pending_ops_before,
            pending_ops_after: self.pending_ops_len(),
            child_fibers: self.child_fiber_count(),
            pure,
            line_effects: output.effects.line.len(),
            audio_commands: output.requests.audio.len(),
            diagnostics: output.diagnostics.len(),
            ..RuntimeStepStats::default()
        }
    }

    fn fail_aot_linear_precondition(&mut self, message: &str, output: &mut RuntimeStepOutput) {
        output
            .diagnostics
            .push(super::RuntimeDiagnostic::new(message.to_owned()));
        self.fiber.frame.status = FlowFiberStatus::Failed(message.to_owned());
    }

    pub(crate) fn can_start_aot_linear_step(
        &self,
        program: &AotProgram,
        input: &RuntimeStepInput,
    ) -> bool {
        input == &RuntimeStepInput::default()
            && self.root.is_none()
            && !self.has_executor_work()
            && self.plan.stream_plans.is_empty()
            && self.fiber.frame.pending_ops.len() == 1
            && self.fiber.frame.control_stack.is_empty()
            && matches!(self.fiber.frame.status, FlowFiberStatus::Running)
            && self
                .fiber
                .frame
                .function_root_position()
                .is_some_and(|cursor| {
                    (match cursor.owner() {
                        crate::plan::code::RuntimeCodeOwner::Function(site) if cursor.is_root() => {
                            program.flow_block(site)
                        }
                        _ => None,
                    })
                    .and_then(|flow| flow.linear_op(cursor.operation_index()?))
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
            AotLinearOp::Let { pattern, expr } => {
                self.evaluate_let_with_backend(pattern, expr, output, pure_backend);
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::Assign { place, value } => {
                match self.evaluate_expr_with_backend(value, pure_backend) {
                    Ok(value) => match self.fiber.frame.env.assign_place(place, value) {
                        Ok(_) => self.advance_aot_linear_cursor(next_op_index),
                        Err(error) => self.fail_eval(error.into_parts().0, output),
                    },
                    Err(error) => self.fail_eval(error, output),
                }
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
            AotLinearOp::EvaluatedEffect(effect) => {
                let mut drop_policy = None;
                if !self.execute_evaluated_effect(effect, &mut drop_policy, output, pure_backend) {
                    return;
                }
                // The private AOT constructor excludes activation-policy and
                // control effects; those retain the native scheduler path.
                debug_assert!(drop_policy.is_none());
                if !self.apply_control_effects(output, pure_backend) {
                    self.advance_aot_linear_cursor(next_op_index);
                }
            }
            AotLinearOp::RegisterCleanup { key, effect } => {
                self.register_scope_cleanup(key.clone(), effect.clone());
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::CancelCleanup { key } => {
                self.cancel_scope_cleanup(key);
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::EnterScope { identity } => {
                self.push_scope_frame(identity.clone());
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::ExitScope => {
                if let Err(error) = self.exit_emitted_scope(output, pure_backend) {
                    self.fail_eval(error, output);
                    return;
                }
                self.advance_aot_linear_cursor(next_op_index);
            }
            AotLinearOp::ExitScopeBind { pattern, expr } => {
                match self.evaluate_expr_with_backend(expr, pure_backend) {
                    Ok(value) => {
                        if let Err(error) = self.exit_emitted_scope(output, pure_backend) {
                            self.fail_eval(error, output);
                            return;
                        }
                        self.bind_value(pattern, value, output);
                        self.advance_aot_linear_cursor(next_op_index);
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            AotLinearOp::Noop => self.advance_aot_linear_cursor(next_op_index),
        }
    }

    fn advance_aot_linear_cursor(&mut self, next_op_index: usize) {
        if let Some(cursor) = self.fiber.frame.function_root_position_mut() {
            assert!(
                cursor.set_operation(next_op_index),
                "AOT operation remains inside original code"
            );
        }
    }
}
