use super::{
    Engine, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr, RuntimePattern, RuntimePayload,
    RuntimeStepOutput, RuntimeStreamEvent, RuntimeValue, StreamMatchArm, StreamOp, StreamRuntimeId,
    StreamRuntimeState, match_runtime_pattern, runtime_value_label,
};
use crate::pure::RuntimeCallBackend;
use crate::stream::StreamEventKind;

#[derive(Clone, Copy, Debug)]
pub(super) struct StreamForNext<'a> {
    stream: &'a StreamRuntimeId,
    pattern: &'a RuntimePattern,
    source: &'a RuntimeExpr,
    body: &'a [StreamOp],
}

impl Engine {
    pub(super) fn step_stream_plans(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let plan = std::sync::Arc::clone(&self.plan);
        for plan in plan.stream_plans() {
            let mut budget = 64usize;
            if !self.execute_stream_ops(plan.id(), plan.ops(), &mut budget, output, pure_backend) {
                continue;
            }
            if budget == 0 {
                output.diagnostics.push(RuntimeDiagnostic::new(format!(
                    "stream {} exhausted frame budget",
                    plan.id().public_label()
                )));
            }
        }
    }

    pub(super) fn execute_stream_ops(
        &mut self,
        stream: &StreamRuntimeId,
        ops: &[StreamOp],
        budget: &mut usize,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        for op in ops {
            if *budget == 0 {
                return true;
            }
            *budget -= 1;
            if !self.execute_stream_op(stream, op, budget, output, pure_backend) {
                return false;
            }
        }
        true
    }

    pub(super) fn execute_stream_op(
        &mut self,
        stream: &StreamRuntimeId,
        op: &StreamOp,
        budget: &mut usize,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        match op {
            StreamOp::Let { pattern, expr } => {
                self.bind_stream_let(pattern, expr, output, pure_backend)
            }
            StreamOp::ForNext {
                pattern,
                source,
                body,
            } => self.execute_stream_for_next(
                StreamForNext {
                    stream,
                    pattern,
                    source,
                    body,
                },
                budget,
                output,
                pure_backend,
            ),
            StreamOp::Yield { expr } => self.yield_stream_item(stream, expr, output, pure_backend),
            StreamOp::If {
                condition,
                then_ops,
                else_ops,
            } => match self.evaluate_bool_with_backend(condition, pure_backend) {
                Ok(true) => self.execute_stream_ops(stream, then_ops, budget, output, pure_backend),
                Ok(false) => {
                    self.execute_stream_ops(stream, else_ops, budget, output, pure_backend)
                }
                Err(error) => {
                    Self::diagnose_runtime_error(error, output);
                    true
                }
            },
            StreamOp::Match { scrutinee, arms } => {
                self.execute_stream_match(stream, scrutinee, arms, budget, output, pure_backend)
            }
            StreamOp::Close { source } => {
                self.close_stream_source(source, output);
                true
            }
            StreamOp::Return => false,
        }
    }

    pub(super) fn bind_stream_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        match self.evaluate_expr_with_backend(expr, pure_backend) {
            Ok(value) => match self.try_bind_pattern(pattern, &value) {
                Ok(true) => true,
                Ok(false) => {
                    output.diagnostics.push(RuntimeDiagnostic::new(format!(
                        "stream pattern did not match {}",
                        runtime_value_label(&value)
                    )));
                    true
                }
                Err(error) => {
                    Self::diagnose_runtime_error(error, output);
                    true
                }
            },
            Err(error) => {
                Self::diagnose_runtime_error(error, output);
                true
            }
        }
    }

    pub(super) fn execute_stream_for_next(
        &mut self,
        args: StreamForNext<'_>,
        budget: &mut usize,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Ok(source_key) = self.evaluate_queue_target_with_backend(args.source, pure_backend)
        else {
            return true;
        };
        while let Some(item) = self.pop_queue_item(&source_key) {
            match match_runtime_pattern(&self.plan, args.pattern, item.value()) {
                Ok(Some(bindings)) => {
                    let should_continue = self.with_temp_bindings(bindings, |this| {
                        this.execute_stream_ops(
                            args.stream,
                            args.body,
                            budget,
                            output,
                            pure_backend,
                        )
                    });
                    if !should_continue {
                        return false;
                    }
                }
                Ok(None) => output.diagnostics.push(RuntimeDiagnostic::new(format!(
                    "stream for-next pattern did not match {source_key}"
                ))),
                Err(error) => Self::diagnose_runtime_error(error, output),
            }
            if *budget == 0 {
                break;
            }
        }
        true
    }

    pub(super) fn yield_stream_item(
        &mut self,
        stream: &StreamRuntimeId,
        expr: &RuntimeExpr,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        match self.evaluate_expr_with_backend(expr, pure_backend) {
            Ok(value) => {
                let item: RuntimePayload = value.into();
                let state = self
                    .fiber
                    .stream_states
                    .entry(stream.clone())
                    .or_insert_with(|| StreamRuntimeState::new(stream.clone()));
                let sequence = state.push_item(item.clone());
                output.effects.stream_events.push(RuntimeStreamEvent {
                    stream: stream.clone(),
                    sequence,
                    kind: StreamEventKind::Item(item),
                });
            }
            Err(error) => Self::diagnose_runtime_error(error, output),
        }
        true
    }

    pub(super) fn execute_stream_match(
        &mut self,
        stream: &StreamRuntimeId,
        scrutinee: &RuntimeExpr,
        arms: &[StreamMatchArm],
        budget: &mut usize,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let value = match self.evaluate_expr_with_backend(scrutinee, pure_backend) {
            Ok(value) => value,
            Err(error) => {
                Self::diagnose_runtime_error(error, output);
                return true;
            }
        };
        for arm in arms {
            let Ok(Some(bindings)) = match_runtime_pattern(&self.plan, &arm.pattern, &value) else {
                continue;
            };
            let guard_matches = if let Some(guard) = arm.guard.as_ref() {
                self.with_temp_bindings_ref(&bindings, |this| {
                    this.evaluate_bool_with_backend(guard, pure_backend)
                })
            } else {
                Ok(true)
            };
            if matches!(guard_matches, Ok(true)) {
                return self.with_temp_bindings(bindings, |this| {
                    this.execute_stream_ops(stream, &arm.ops, budget, output, pure_backend)
                });
            }
            if let Err(error) = guard_matches {
                Self::diagnose_runtime_error(error, output);
            }
        }
        true
    }

    pub(super) fn close_stream_source(
        &mut self,
        source: &RuntimeExpr,
        output: &mut RuntimeStepOutput,
    ) {
        match self.evaluate_queue_target(source) {
            Ok(target) => {
                if let Some(stream) = target.strip_prefix("stream:")
                    && let Ok(stream_id) = StreamRuntimeId::from_runtime_target_value(stream)
                    && let Some(state) = self.fiber.stream_states.get_mut(&stream_id)
                    && let Some(sequence) = state.close_with_sequence()
                {
                    output.effects.stream_events.push(RuntimeStreamEvent {
                        stream: stream_id,
                        sequence,
                        kind: StreamEventKind::End,
                    });
                }
            }
            Err(error) => Self::diagnose_runtime_error(error, output),
        }
    }

    pub(super) fn evaluate_queue_target(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<String, RuntimeEvalError> {
        let mut pure_backend = crate::pure::VmRuntimePureCallBackend::default();
        self.evaluate_queue_target_with_backend(expr, &mut pure_backend)
    }

    pub(super) fn evaluate_queue_target_with_backend(
        &mut self,
        expr: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<String, RuntimeEvalError> {
        match self.evaluate_expr_with_backend(expr, pure_backend)? {
            RuntimeValue::EntityRef(target) => {
                let target = target.runtime_label();
                let stream = StreamRuntimeId::from_runtime_target_value(&target)
                    .map_err(|_| RuntimeEvalError::ExpectedEntityRef(target))?;
                Ok(format!("stream:{}", stream.canonical_label()))
            }
            RuntimeValue::String(target) => {
                let stream = StreamRuntimeId::from_runtime_target_value(&target)
                    .map_err(|_| RuntimeEvalError::ExpectedEntityRef(target))?;
                Ok(format!("stream:{}", stream.canonical_label()))
            }
            value => Err(RuntimeEvalError::ExpectedEntityRef(runtime_value_label(
                &value,
            ))),
        }
    }

    pub(super) fn pop_queue_item(&mut self, key: &str) -> Option<RuntimePayload> {
        key.strip_prefix("stream:")
            .and_then(|stream| StreamRuntimeId::from_runtime_target_value(stream).ok())
            .and_then(|stream| {
                self.fiber
                    .stream_states
                    .get_mut(&stream)
                    .and_then(|state| state.queue.pop_front())
            })
    }
}
