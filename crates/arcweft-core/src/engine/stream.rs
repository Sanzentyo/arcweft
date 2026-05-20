use super::{
    Engine, RuntimeDiagnostic, RuntimeEvalError, RuntimeExpr, RuntimePattern, RuntimePayload,
    RuntimeStepOutput, RuntimeStreamEvent, RuntimeValue, SourceEventKind, SourceId, StreamMatchArm,
    StreamOp, StreamRuntimeId, StreamRuntimeState, match_runtime_pattern, runtime_value_label,
};

impl Engine {
    pub(super) fn step_stream_plans(&mut self, output: &mut RuntimeStepOutput) {
        let stream_plans = self.plan.stream_plans.clone();
        for plan in stream_plans {
            let mut budget = 64usize;
            if !self.execute_stream_ops(&plan.id, &plan.ops, &mut budget, output) {
                continue;
            }
            if budget == 0 {
                output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("stream {} exhausted frame budget", plan.id.0),
                });
            }
        }
    }

    pub(super) fn execute_stream_ops(
        &mut self,
        stream: &StreamRuntimeId,
        ops: &[StreamOp],
        budget: &mut usize,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        for op in ops {
            if *budget == 0 {
                return true;
            }
            *budget -= 1;
            if !self.execute_stream_op(stream, op, budget, output) {
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
    ) -> bool {
        match op {
            StreamOp::Let { pattern, expr } => self.bind_stream_let(pattern, expr, output),
            StreamOp::ForNext {
                pattern,
                source,
                body,
            } => self.execute_stream_for_next(stream, pattern, source, body, budget, output),
            StreamOp::Yield { expr } => self.yield_stream_item(stream, expr, output),
            StreamOp::If {
                condition,
                then_ops,
                else_ops,
            } => match self.evaluate_bool(condition) {
                Ok(true) => self.execute_stream_ops(stream, then_ops, budget, output),
                Ok(false) => self.execute_stream_ops(stream, else_ops, budget, output),
                Err(error) => {
                    Self::diagnose_runtime_error(error, output);
                    true
                }
            },
            StreamOp::Match { scrutinee, arms } => {
                self.execute_stream_match(stream, scrutinee, arms, budget, output)
            }
            StreamOp::Close { source } => {
                self.close_stream_source(source, output);
                true
            }
            StreamOp::Return => false,
            StreamOp::Noop => true,
        }
    }

    pub(super) fn bind_stream_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        match self.evaluate_expr(expr) {
            Ok(value) => match self.try_bind_pattern(pattern, &value) {
                Ok(true) => true,
                Ok(false) => {
                    output.diagnostics.push(RuntimeDiagnostic {
                        message: format!(
                            "stream pattern did not match {}",
                            runtime_value_label(&value)
                        ),
                    });
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
        stream: &StreamRuntimeId,
        pattern: &RuntimePattern,
        source: &RuntimeExpr,
        body: &[StreamOp],
        budget: &mut usize,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let Ok(source_key) = self.evaluate_queue_target(source) else {
            return true;
        };
        while let Some(item) = self.pop_queue_item(&source_key) {
            let previous = self.fiber.env.clone();
            self.fiber.env.push_scope();
            match match_runtime_pattern(pattern, item.value()) {
                Ok(Some(bindings)) => {
                    self.fiber.env.bind_all(bindings);
                    if !self.execute_stream_ops(stream, body, budget, output) {
                        self.fiber.env = previous;
                        return false;
                    }
                }
                Ok(None) => output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("stream for-next pattern did not match {source_key}"),
                }),
                Err(error) => Self::diagnose_runtime_error(error, output),
            }
            self.fiber.env = previous;
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
    ) -> bool {
        match self.evaluate_expr(expr) {
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
                    kind: SourceEventKind::Item(item),
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
    ) -> bool {
        let value = match self.evaluate_expr(scrutinee) {
            Ok(value) => value,
            Err(error) => {
                Self::diagnose_runtime_error(error, output);
                return true;
            }
        };
        for arm in arms {
            let Ok(Some(bindings)) = match_runtime_pattern(&arm.pattern, &value) else {
                continue;
            };
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings);
            let guard_matches = arm
                .guard
                .as_ref()
                .map_or(Ok(true), |guard| self.evaluate_bool(guard));
            if matches!(guard_matches, Ok(true)) {
                let should_continue = self.execute_stream_ops(stream, &arm.ops, budget, output);
                self.fiber.env = previous;
                return should_continue;
            }
            if let Err(error) = guard_matches {
                Self::diagnose_runtime_error(error, output);
            }
            self.fiber.env = previous;
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
                if let Some(source) = target.strip_prefix("source:") {
                    self.close_source(&SourceId(source.to_owned()), output);
                } else if let Some(stream) = target.strip_prefix("stream:") {
                    if let Some(state) = self
                        .fiber
                        .stream_states
                        .get_mut(&StreamRuntimeId(stream.to_owned()))
                    {
                        state.close();
                    }
                }
            }
            Err(error) => Self::diagnose_runtime_error(error, output),
        }
    }

    pub(super) fn evaluate_queue_target(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<String, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::EntityRef(target) | RuntimeValue::String(target) => {
                if self
                    .fiber
                    .source_states
                    .contains_key(&SourceId(target.clone()))
                {
                    Ok(format!("source:{target}"))
                } else if self
                    .fiber
                    .stream_states
                    .contains_key(&StreamRuntimeId(target.clone()))
                {
                    Ok(format!("stream:{target}"))
                } else {
                    Ok(format!("source:{target}"))
                }
            }
            value => Err(RuntimeEvalError::ExpectedEntityRef(runtime_value_label(
                &value,
            ))),
        }
    }

    pub(super) fn pop_queue_item(&mut self, key: &str) -> Option<RuntimePayload> {
        if let Some(source) = key.strip_prefix("source:") {
            return self
                .fiber
                .source_states
                .get_mut(&SourceId(source.to_owned()))
                .and_then(|state| state.queue.pop_front());
        }
        key.strip_prefix("stream:").and_then(|stream| {
            self.fiber
                .stream_states
                .get_mut(&StreamRuntimeId(stream.to_owned()))
                .and_then(|state| state.queue.pop_front())
        })
    }
}
