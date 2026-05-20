use super::{
    Engine, LineEffectRequest, RuntimeBinding, RuntimeDiagnostic, RuntimeStepOutput, RuntimeValue,
    SourceEvent, SourceEventKind, SourceHandlerPlan, SourceId, SourceOp, SourcePlan, SourcePolicy,
    SourceRuntimeState, match_runtime_pattern, runtime_value_label,
};

impl Engine {
    pub(super) fn apply_source_events(
        &mut self,
        events: Vec<SourceEvent<String, String>>,
        output: &mut RuntimeStepOutput,
    ) {
        for event in events {
            output.effects.source_events.push(event.clone());
            let plan = self
                .plan
                .source_plans
                .iter()
                .find(|plan| plan.id == event.source)
                .cloned();
            if let Some(plan) = plan {
                self.dispatch_source_event(&plan, event, output);
            } else {
                self.apply_unhandled_source_event(event, output);
            }
        }
    }

    pub(super) fn dispatch_source_event(
        &mut self,
        plan: &SourcePlan,
        event: SourceEvent<String, String>,
        output: &mut RuntimeStepOutput,
    ) {
        self.record_source_event_state(&event, output);
        let mut handled = false;
        for handler in &plan.handlers {
            let Some((bindings, ops)) = source_handler_match(handler, &event.kind) else {
                continue;
            };
            handled = true;
            self.execute_source_ops(&plan.id, ops, bindings, output);
        }
        if !handled && matches!(event.kind, SourceEventKind::Item(_)) {
            self.apply_unhandled_source_event(event, output);
        }
    }

    pub(super) fn apply_unhandled_source_event(
        &mut self,
        event: SourceEvent<String, String>,
        output: &mut RuntimeStepOutput,
    ) {
        let state = self
            .fiber
            .source_states
            .entry(event.source.clone())
            .or_insert_with(|| {
                SourceRuntimeState::new(event.source.clone(), SourcePolicy::default())
            });
        if let Some(message) = state.apply_event(event) {
            output.diagnostics.push(RuntimeDiagnostic { message });
        }
    }

    pub(super) fn record_source_event_state(
        &mut self,
        event: &SourceEvent<String, String>,
        output: &mut RuntimeStepOutput,
    ) {
        let state = self
            .fiber
            .source_states
            .entry(event.source.clone())
            .or_insert_with(|| {
                SourceRuntimeState::new(event.source.clone(), SourcePolicy::default())
            });
        match &event.kind {
            SourceEventKind::Error(error) => {
                state.last_error = Some(error.clone());
                output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("source {} error: {error}", state.id.0),
                });
            }
            SourceEventKind::Disconnected
            | SourceEventKind::PermissionRevoked
            | SourceEventKind::End => state.close(),
            SourceEventKind::Item(_) | SourceEventKind::Progress(_) => {}
        }
    }

    pub(super) fn execute_source_ops(
        &mut self,
        source: &SourceId,
        ops: &[SourceOp],
        bindings: Vec<RuntimeBinding>,
        output: &mut RuntimeStepOutput,
    ) {
        let previous = self.fiber.env.clone();
        self.fiber.env.push_scope();
        self.fiber.env.bind_all(bindings);
        for op in ops {
            self.execute_source_op(source, op, output);
        }
        self.fiber.env = previous;
    }

    pub(super) fn execute_source_op(
        &mut self,
        source: &SourceId,
        op: &SourceOp,
        output: &mut RuntimeStepOutput,
    ) {
        match op {
            SourceOp::Yield(expr) => match self.evaluate_expr(expr) {
                Ok(value) => self.push_source_item(source, runtime_value_label(&value), output),
                Err(error) => Self::diagnose_runtime_error(error, output),
            },
            SourceOp::Effect(effect) => output.effects.line.push(effect.clone()),
            SourceOp::SignalWrite(write) => output
                .effects
                .line
                .push(LineEffectRequest::SignalWrite(write.clone())),
            SourceOp::Log(log) => output
                .effects
                .line
                .push(LineEffectRequest::Log(log.clone())),
            SourceOp::Close(target) => self.close_source(target, output),
            SourceOp::Noop => {}
        }
    }

    pub(super) fn push_source_item(
        &mut self,
        source: &SourceId,
        item: String,
        output: &mut RuntimeStepOutput,
    ) {
        let state = self
            .fiber
            .source_states
            .entry(source.clone())
            .or_insert_with(|| SourceRuntimeState::new(source.clone(), SourcePolicy::default()));
        if let Some(message) = state.push_item(item) {
            output.diagnostics.push(RuntimeDiagnostic { message });
        }
    }

    pub(super) fn close_source(&mut self, source: &SourceId, output: &mut RuntimeStepOutput) {
        if let Some(state) = self.fiber.source_states.get_mut(source) {
            state.close();
        }
        output.requests.source_close.push(source.clone());
    }
}

fn source_handler_match<'a>(
    handler: &'a SourceHandlerPlan,
    event: &SourceEventKind<String, String>,
) -> Option<(Vec<RuntimeBinding>, &'a [SourceOp])> {
    match (handler, event) {
        (SourceHandlerPlan::Item { pattern, ops }, SourceEventKind::Item(item))
        | (SourceHandlerPlan::Error { pattern, ops }, SourceEventKind::Error(item))
        | (SourceHandlerPlan::Progress { pattern, ops }, SourceEventKind::Progress(item)) => {
            let bindings = match_runtime_pattern(pattern, &RuntimeValue::String(item.clone()))
                .ok()
                .flatten()?;
            Some((bindings, ops))
        }
        (SourceHandlerPlan::Disconnected { ops }, SourceEventKind::Disconnected)
        | (SourceHandlerPlan::PermissionRevoked { ops }, SourceEventKind::PermissionRevoked)
        | (SourceHandlerPlan::End { ops }, SourceEventKind::End) => Some((Vec::new(), ops)),
        _ => None,
    }
}
