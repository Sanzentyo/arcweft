use super::{
    Engine, LineEffectRequest, RuntimeBinding, RuntimeDiagnostic, RuntimePayload,
    RuntimeSourceEvent, RuntimeStepOutput, SourceEventKind, SourceHandlerPlan, SourceId, SourceOp,
    SourcePlan, SourcePolicy, SourceRuntimeState, match_runtime_pattern,
};
use crate::pure::RuntimeCallBackend;

impl Engine {
    pub(super) fn apply_source_events(
        &mut self,
        events: Vec<RuntimeSourceEvent>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
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
                self.dispatch_source_event(&plan, event, output, pure_backend);
            } else {
                self.apply_unhandled_source_event(event, output);
            }
        }
    }

    pub(super) fn dispatch_source_event(
        &mut self,
        plan: &SourcePlan,
        event: RuntimeSourceEvent,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        self.record_source_event_state(&event, output);
        let mut handled = false;
        for handler in &plan.handlers {
            let Some((bindings, ops)) = source_handler_match(handler, &event.kind) else {
                continue;
            };
            handled = true;
            self.execute_source_ops(&plan.id, ops, bindings, output, pure_backend);
        }
        if !handled && matches!(event.kind, SourceEventKind::Item(_)) {
            self.apply_unhandled_source_event(event, output);
        }
    }

    pub(super) fn apply_unhandled_source_event(
        &mut self,
        event: RuntimeSourceEvent,
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
            output.diagnostics.push(RuntimeDiagnostic::new(message));
        }
    }

    pub(super) fn record_source_event_state(
        &mut self,
        event: &RuntimeSourceEvent,
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
                output.diagnostics.push(RuntimeDiagnostic::new(format!(
                    "source {} error: {}",
                    state.id.0,
                    error.label()
                )));
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
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        self.with_temp_bindings(bindings, |this| {
            for op in ops {
                this.execute_source_op(source, op, output, pure_backend);
            }
        });
    }

    pub(super) fn execute_source_op(
        &mut self,
        source: &SourceId,
        op: &SourceOp,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        match op {
            SourceOp::Yield(expr) => match self.evaluate_expr_with_backend(expr, pure_backend) {
                Ok(value) => self.push_source_item(source, value.into(), output),
                Err(error) => Self::diagnose_runtime_error(error, output),
            },
            SourceOp::Effect(effect) => {
                self.emit_line_effect(effect.clone(), output, pure_backend);
            }
            SourceOp::SignalWrite(write) => self.emit_line_effect(
                LineEffectRequest::SignalWrite(write.clone()),
                output,
                pure_backend,
            ),
            SourceOp::Log(log) => {
                self.emit_line_effect(LineEffectRequest::Log(log.clone()), output, pure_backend);
            }
            SourceOp::Close(target) => self.close_source(target, output),
            SourceOp::Noop => {}
        }
    }

    pub(super) fn push_source_item(
        &mut self,
        source: &SourceId,
        item: RuntimePayload,
        output: &mut RuntimeStepOutput,
    ) {
        let state = self
            .fiber
            .source_states
            .entry(source.clone())
            .or_insert_with(|| SourceRuntimeState::new(source.clone(), SourcePolicy::default()));
        if let Some(message) = state.push_item(item) {
            output.diagnostics.push(RuntimeDiagnostic::new(message));
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
    event: &SourceEventKind<RuntimePayload, RuntimePayload>,
) -> Option<(Vec<RuntimeBinding>, &'a [SourceOp])> {
    match (handler, event) {
        (SourceHandlerPlan::Item { pattern, ops }, SourceEventKind::Item(item))
        | (SourceHandlerPlan::Error { pattern, ops }, SourceEventKind::Error(item)) => {
            let bindings = match_runtime_pattern(pattern, item.value())
                .ok()
                .flatten()?;
            Some((bindings, ops))
        }
        (SourceHandlerPlan::Progress { pattern, ops }, SourceEventKind::Progress(item)) => {
            let payload = RuntimePayload::from(item.as_str());
            let bindings = match_runtime_pattern(pattern, payload.value())
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
