use super::{
    AwaitManyInFlight, AwaitManyState, AwaitManyTarget, AwaitState, AwaitTarget, AwbcContentUnitId,
    AwbcFrameSlotRole, AwbcFunctionId, AwbcHostCallId, AwbcHostCallMode, AwbcProductStepExecutor,
    AwbcResumePointId, AwbcSourceEventKind, AwbcSourcePlanId, AwbcTaskPlanId, AwbcTrapCode,
    ChoiceRuntimeOption, ChoiceState, DialogueState, FiberStatus, FiberSuspensionReason,
    FiberTerminalValue, FiberTrap, FlowExit, FlowFiberStatus, HostCallState,
    HostTaskRequestTemplate, LogicalDuration, MappedEffect, NeedId, ProductStepError,
    RuntimeBinding, RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId,
    RuntimeHostCallMode, RuntimeHostCallTarget, RuntimePayload, RuntimeSourceEvent,
    RuntimeStepMode, RuntimeStepOptions, RuntimeStepOutput, RuntimeStepStopReason, RuntimeValue,
    SourceEventKind, SourceId, SourceRuntimeState, TaskId, flow_id_from_awbc_public_id,
    has_host_requests, has_visible_output, line_id_from_awbc_public_id,
    runtime_sequence_from_literal_values, runtime_value_label, source_diagnostic, source_id_for,
};
use crate::source::SourcePolicy;

impl AwbcProductStepExecutor {
    pub(super) fn entry_targets_active_function(&self) -> bool {
        let Some(entry) = self.program.entries.get(self.fiber.entry.index()) else {
            return false;
        };
        let Some(entry_function) = entry.target.function() else {
            return false;
        };
        self.fiber
            .frames
            .first()
            .is_some_and(|frame| frame.function == entry_function)
    }

    pub(super) fn apply_source_events(
        &mut self,
        events: Vec<RuntimeSourceEvent>,
        output: &mut RuntimeStepOutput,
    ) {
        for event in events {
            if let Some(plan_id) = self.source_plan_for_id(&event.source) {
                self.record_source_event_state(&event, output);
                self.sync_compact_source_state(plan_id);
                let handled = self.spawn_source_handler(plan_id, &event, output);
                if !handled && matches!(event.kind, SourceEventKind::Item(_)) {
                    self.apply_unhandled_source_event(event.clone(), output);
                    self.sync_compact_source_state(plan_id);
                }
            } else {
                self.apply_unhandled_source_event(event.clone(), output);
            }
            output.effects.source_events.push(event);
        }
    }

    pub(super) fn record_source_event_state(
        &mut self,
        event: &RuntimeSourceEvent,
        output: &mut RuntimeStepOutput,
    ) {
        let state = self
            .facade_fiber
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

    pub(super) fn apply_unhandled_source_event(
        &mut self,
        event: RuntimeSourceEvent,
        output: &mut RuntimeStepOutput,
    ) {
        let state = self
            .facade_fiber
            .source_states
            .entry(event.source.clone())
            .or_insert_with(|| {
                SourceRuntimeState::new(event.source.clone(), SourcePolicy::default())
            });
        if let Some(message) = state.apply_event(event) {
            output.diagnostics.push(RuntimeDiagnostic::new(message));
        }
    }

    pub(super) fn sync_compact_source_state(&mut self, plan: AwbcSourcePlanId) {
        let id = source_id_for(&self.program, plan);
        let Some(runtime) = self.facade_fiber.source_states.get(&id) else {
            return;
        };
        let Some(compact) = self
            .fiber
            .sources
            .iter_mut()
            .find(|state| state.plan == plan)
        else {
            return;
        };
        compact.queue = runtime
            .queue
            .iter()
            .map(|payload| payload.value().clone())
            .collect();
        compact.closed = runtime.closed;
        compact.last_error = runtime
            .last_error
            .as_ref()
            .map(|payload| payload.value().clone());
        compact.overflow_count = runtime.overflow_count;
    }

    pub(super) fn spawn_source_handler(
        &mut self,
        plan: AwbcSourcePlanId,
        event: &RuntimeSourceEvent,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let Some(source) = self.program.source_plans.get(plan.index()) else {
            return false;
        };
        let kind = match event.kind {
            SourceEventKind::Item(_) => AwbcSourceEventKind::Item,
            SourceEventKind::Error(_) => AwbcSourceEventKind::Error,
            SourceEventKind::Progress(_) => AwbcSourceEventKind::Progress,
            SourceEventKind::Disconnected => AwbcSourceEventKind::Disconnected,
            SourceEventKind::PermissionRevoked => AwbcSourceEventKind::PermissionRevoked,
            SourceEventKind::End => AwbcSourceEventKind::End,
        };
        let Some(handler) = source.handlers.iter().find(|handler| handler.kind == kind) else {
            return false;
        };
        let args = match &event.kind {
            SourceEventKind::Item(value) | SourceEventKind::Error(value) => {
                vec![value.value().clone()]
            }
            SourceEventKind::Progress(value) => vec![RuntimeValue::String(value.clone())],
            SourceEventKind::Disconnected
            | SourceEventKind::PermissionRevoked
            | SourceEventKind::End => Vec::new(),
        };
        if let (Some(pattern), Some(value)) = (handler.pattern, args.first()) {
            match crate::awbc::vm::test_pattern(&self.program, pattern, value) {
                Ok(true) => {}
                Ok(false) => return false,
                Err(error) => {
                    self.record_error(ProductStepError::Internal(error.to_string()), output);
                    return false;
                }
            }
        }
        self.spawn_child(handler.function, &args, output);
        true
    }

    pub(super) fn resume_at(
        &mut self,
        resume: AwbcResumePointId,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        match self.fiber.resume_at(&self.program, resume) {
            Ok(()) => true,
            Err(error) => {
                self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                false
            }
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn fail_with_error(
        &mut self,
        error: ProductStepError,
        output: &mut RuntimeStepOutput,
    ) {
        let message = error.to_string();
        output.diagnostics.push(RuntimeDiagnostic::categorized(
            error.category(),
            message.clone(),
        ));
        self.fiber.mark_trapped(FiberTrap {
            code: match error {
                ProductStepError::Type(_) => AwbcTrapCode::TypeMismatch,
                ProductStepError::Host(_) => AwbcTrapCode::HostAbiMismatch,
                ProductStepError::Input(_) | ProductStepError::Internal(_) => {
                    AwbcTrapCode::InternalInvariant
                }
            },
            message: Some(message),
            source_map: None,
        });
    }

    pub(super) fn fail_with_trap(
        &mut self,
        code: AwbcTrapCode,
        message: String,
        source_map: Option<crate::awbc::schema::AwbcSourceMapId>,
        output: &mut RuntimeStepOutput,
    ) {
        let trap = FiberTrap {
            code,
            message: Some(message),
            source_map,
        };
        self.record_trap(&trap, output);
        self.fiber.mark_trapped(trap);
    }

    #[allow(clippy::needless_pass_by_value, clippy::unused_self)]
    pub(super) fn record_error(&self, error: ProductStepError, output: &mut RuntimeStepOutput) {
        output.diagnostics.push(RuntimeDiagnostic::categorized(
            error.category(),
            error.to_string(),
        ));
    }

    pub(super) fn record_trap(&self, trap: &FiberTrap, output: &mut RuntimeStepOutput) {
        let category = match trap.code {
            AwbcTrapCode::TypeMismatch => RuntimeDiagnosticCategory::Type,
            AwbcTrapCode::PatternMismatch => RuntimeDiagnosticCategory::Pattern,
            AwbcTrapCode::HostAbiMismatch => RuntimeDiagnosticCategory::Host,
            AwbcTrapCode::CapabilityDenied => RuntimeDiagnosticCategory::Capability,
            AwbcTrapCode::DivisionByZero
            | AwbcTrapCode::InvalidIndex
            | AwbcTrapCode::MissingDynamicTarget
            | AwbcTrapCode::ExplicitPanic
            | AwbcTrapCode::UninitializedRegister => RuntimeDiagnosticCategory::Runtime,
            AwbcTrapCode::InternalInvariant => RuntimeDiagnosticCategory::Internal,
        };
        let message = trap
            .message
            .clone()
            .unwrap_or_else(|| format!("AWBC trap {:?}", trap.code));
        let diagnostic = source_diagnostic(&self.program, trap.source_map, category, message);
        if !output.diagnostics.contains(&diagnostic) {
            output.diagnostics.push(diagnostic);
        }
    }

    pub(super) fn stop_reason(
        &self,
        options: RuntimeStepOptions,
        executed_ops: usize,
        output: &RuntimeStepOutput,
    ) -> RuntimeStepStopReason {
        if self.fiber.status == FiberStatus::Trapped {
            return RuntimeStepStopReason::Failed;
        }
        if self.fiber.status == FiberStatus::Returned && self.child_fibers.is_empty() {
            return RuntimeStepStopReason::Done;
        }
        if matches!(
            self.fiber
                .suspension
                .as_ref()
                .map(|suspension| &suspension.reason),
            Some(FiberSuspensionReason::BudgetYield)
        ) {
            return RuntimeStepStopReason::BudgetExhausted;
        }
        if self.fiber.status == FiberStatus::Suspended {
            return if has_visible_output(output) || has_host_requests(output) {
                RuntimeStepStopReason::Output
            } else {
                RuntimeStepStopReason::Blocked
            };
        }
        if options.mode == RuntimeStepMode::Game && has_visible_output(output) {
            return RuntimeStepStopReason::Output;
        }
        if options.mode == RuntimeStepMode::OneOp && executed_ops > 0 {
            return RuntimeStepStopReason::OneOp;
        }
        if executed_ops >= options.budget.max_ops {
            return RuntimeStepStopReason::BudgetExhausted;
        }
        if has_host_requests(output) {
            RuntimeStepStopReason::Output
        } else {
            RuntimeStepStopReason::OneOp
        }
    }

    pub(super) fn should_return_to_host(
        &self,
        mode: RuntimeStepMode,
        output: &RuntimeStepOutput,
        executed_ops: usize,
    ) -> bool {
        if self.fiber.status == FiberStatus::Trapped {
            return true;
        }
        if self.fiber.status == FiberStatus::Returned && self.child_fibers.is_empty() {
            return true;
        }
        if matches!(
            self.fiber
                .suspension
                .as_ref()
                .map(|suspension| &suspension.reason),
            Some(FiberSuspensionReason::BudgetYield)
        ) {
            return true;
        }
        if self.fiber.status == FiberStatus::Suspended {
            return true;
        }
        match mode {
            RuntimeStepMode::OneOp => executed_ops > 0,
            RuntimeStepMode::Game => has_visible_output(output),
            RuntimeStepMode::Drain | RuntimeStepMode::Server => false,
        }
    }

    pub(super) fn sync_facade(&mut self) {
        if let Ok(frame) = self.fiber.active_frame()
            && let Some(layout) = self.program.frame_layouts.get(frame.layout.index())
        {
            let active_scope_count = frame.scopes.len();
            let mut scopes = vec![Vec::new(); active_scope_count.saturating_add(1)];
            for (index, slot) in layout.slots.iter().enumerate() {
                let slot_depth = usize::try_from(slot.scope_depth).unwrap_or(usize::MAX);
                if !matches!(
                    slot.role,
                    AwbcFrameSlotRole::Parameter | AwbcFrameSlotRole::Local
                ) || slot_depth > active_scope_count
                {
                    continue;
                }
                let Some(name) = slot
                    .name
                    .and_then(|id| self.program.strings.get(id.index()))
                else {
                    continue;
                };
                if let Some(value) = frame.registers.get(index).and_then(Option::as_ref)
                    && let Some(scope) = scopes.get_mut(slot_depth)
                {
                    scope.push(RuntimeBinding {
                        name: name.clone(),
                        value: value.clone(),
                    });
                }
            }
            self.facade_fiber.env.replace_scopes_with_bindings(scopes);
        }
        self.facade_fiber.line_cursor =
            usize::try_from(self.fiber.line_cursor).unwrap_or(usize::MAX);
        self.facade_fiber.status = self.effective_status();
    }

    pub(super) fn effective_status(&self) -> FlowFiberStatus {
        if !self.child_fibers.is_empty()
            && matches!(
                self.fiber.status,
                FiberStatus::Returned | FiberStatus::Suspended
            )
        {
            return FlowFiberStatus::Running;
        }
        match self.fiber.status {
            FiberStatus::Running => FlowFiberStatus::Running,
            FiberStatus::Returned => match self.fiber.terminal.as_ref() {
                Some(FiberTerminalValue::Returned(Some(value))) => {
                    FlowFiberStatus::Done(FlowExit::Return(runtime_value_label(value)))
                }
                _ => FlowFiberStatus::Done(FlowExit::Done),
            },
            FiberStatus::Trapped => match self.fiber.terminal.as_ref() {
                Some(FiberTerminalValue::Trapped(trap)) => FlowFiberStatus::Failed(
                    trap.message
                        .clone()
                        .unwrap_or_else(|| format!("AWBC trap {:?}", trap.code)),
                ),
                _ => FlowFiberStatus::Failed("AWBC fiber trapped".to_owned()),
            },
            FiberStatus::Suspended => self.suspension_status(),
        }
    }

    pub(super) fn suspension_status(&self) -> FlowFiberStatus {
        let Some(suspension) = self.fiber.suspension.as_ref() else {
            return FlowFiberStatus::Running;
        };
        match &suspension.reason {
            FiberSuspensionReason::Dialogue {
                content,
                line_task_group,
            } => FlowFiberStatus::Dialogue(DialogueState {
                line: line_id_from_awbc_public_id(&self.content_public_id(*content))
                    .expect("AWBC content public ID should be a valid runtime line ID"),
                task_group: line_task_group.index(),
                resume: None,
                started_nodes: self
                    .active_dialogue
                    .as_ref()
                    .map(|active| {
                        active
                            .started_nodes
                            .iter()
                            .map(|node| node.index())
                            .collect()
                    })
                    .unwrap_or_default(),
                elapsed: LogicalDuration::from_nanos(
                    self.active_dialogue
                        .as_ref()
                        .map_or(0, |active| active.elapsed_nanos),
                ),
            }),
            FiberSuspensionReason::Choice { .. } => {
                let active = self.active_choice.as_ref();
                FlowFiberStatus::Choice(ChoiceState {
                    id: active.and_then(|active| active.public_id.clone()),
                    options: active
                        .map(|active| active.options.clone())
                        .unwrap_or_default(),
                    resume: None,
                })
            }
            FiberSuspensionReason::Await { task, .. } => {
                let task = TaskId(runtime_value_label(task));
                let plan = self.task_plan_for_id(&task.0);
                FlowFiberStatus::Waiting(AwaitState {
                    binding: None,
                    target: AwaitTarget::new(
                        plan.map_or_else(|| NeedId(task.0.clone()), |plan| self.task_need_id(plan)),
                        task,
                        HostTaskRequestTemplate::new("awbc", "await", []),
                    ),
                    resume: None,
                })
            }
            FiberSuspensionReason::AwaitMany(state) => {
                let target = AwaitManyTarget::new(
                    self.task_need_id(state.plan),
                    TaskId(self.task_public_id(state.plan)),
                    crate::value::RuntimeExpr::Value(runtime_sequence_from_literal_values(
                        state.items.clone(),
                    )),
                    "item",
                    self.program
                        .task_plans
                        .get(state.plan.index())
                        .and_then(|plan| plan.many.as_ref())
                        .map_or(usize::MAX, |many| many.limit as usize),
                    HostTaskRequestTemplate::new("awbc", "await_many", []),
                );
                FlowFiberStatus::WaitingMany(Box::new(AwaitManyState {
                    binding: None,
                    target,
                    resume: None,
                    items: state.items.clone(),
                    next_index: state.next_index as usize,
                    in_flight: state
                        .in_flight
                        .iter()
                        .map(|item| AwaitManyInFlight {
                            index: item.index as usize,
                            task: TaskId(item.task_id.clone()),
                            need: NeedId(item.need_id.clone()),
                        })
                        .collect(),
                    results: state
                        .results
                        .iter()
                        .cloned()
                        .map(|value| value.map(RuntimePayload::from))
                        .collect(),
                }))
            }
            FiberSuspensionReason::HostCall { call, .. } => self.host_call_status(*call),
            FiberSuspensionReason::BudgetYield => FlowFiberStatus::Running,
        }
    }

    pub(super) fn host_call_status(&self, call: AwbcHostCallId) -> FlowFiberStatus {
        let record = self.program.host_calls.get(call.index());
        let public_id = record
            .and_then(|record| self.program.strings.get(record.public_id.index()))
            .cloned()
            .unwrap_or_else(|| format!("awbc.host_call.{}", call.0));
        let id = self.pending_host_call.as_ref().map_or_else(
            || RuntimeHostCallId(public_id.clone()),
            |pending| pending.id.clone(),
        );
        FlowFiberStatus::HostCall(HostCallState {
            binding: None,
            target: RuntimeHostCallTarget::new(
                public_id,
                record
                    .and_then(|record| self.program.strings.get(record.capability.index()))
                    .cloned()
                    .unwrap_or_else(|| "host".to_owned()),
                record
                    .and_then(|record| self.program.strings.get(record.operation.index()))
                    .cloned()
                    .unwrap_or_else(|| "call".to_owned()),
                [],
                record.map_or(RuntimeHostCallMode::Suspend, |record| match record.mode {
                    AwbcHostCallMode::Immediate => RuntimeHostCallMode::Immediate,
                    AwbcHostCallMode::Suspend => RuntimeHostCallMode::Suspend,
                }),
                record.is_none_or(|record| record.deterministic),
            ),
            id,
            resume: None,
        })
    }

    pub(super) fn choice_runtime_option(
        &self,
        option: &crate::awbc::schema::AwbcChoiceOption,
    ) -> ChoiceRuntimeOption {
        let mut effects = Vec::new();
        for effect in &option.effects {
            if let Some(plan) = self.program.effect_plans.get(effect.index())
                && let MappedEffect::Line(effect) =
                    plan.kind.map_product_effect(&self.program, *effect, &[])
            {
                effects.push(effect);
            }
        }
        let out = option.out_effect.and_then(|effect| {
            let plan = self.program.effect_plans.get(effect.index())?;
            match plan.kind.map_product_effect(&self.program, effect, &[]) {
                MappedEffect::Line(crate::effect::LineEffectRequest::Out(out)) => Some(out),
                _ => None,
            }
        });
        ChoiceRuntimeOption {
            id: option
                .public_id
                .and_then(|id| self.program.strings.get(id.index()).cloned()),
            label: self
                .program
                .strings
                .get(option.label.index())
                .cloned()
                .unwrap_or_else(|| "choice".to_owned()),
            target: option.target.map(|target| {
                let public_id = self.function_public_id(target);
                flow_id_from_awbc_public_id(&public_id)
                    .expect("AWBC function public ID should be a valid runtime flow ID")
            }),
            out,
            effects,
        }
    }

    pub(super) fn content_public_id(&self, content: AwbcContentUnitId) -> String {
        self.program
            .content_units
            .get(content.index())
            .and_then(|content| self.program.strings.get(content.public_id.index()))
            .cloned()
            .unwrap_or_else(|| format!("awbc.content.{}", content.0))
    }

    pub(super) fn function_public_id(&self, function: AwbcFunctionId) -> String {
        self.program
            .functions
            .get(function.index())
            .and_then(|function| function.public_id)
            .and_then(|id| self.program.strings.get(id.index()))
            .cloned()
            .unwrap_or_else(|| format!("awbc.function.{}", function.0))
    }

    pub(super) fn task_plan_for_id(&self, task: &str) -> Option<AwbcTaskPlanId> {
        self.program
            .task_plans
            .iter()
            .enumerate()
            .find_map(|(index, plan)| {
                self.program
                    .strings
                    .get(plan.public_id.index())
                    .filter(|public_id| public_id.as_str() == task)
                    .and_then(|_| u32::try_from(index).ok())
                    .map(AwbcTaskPlanId)
            })
    }

    pub(super) fn task_need_id(&self, plan: AwbcTaskPlanId) -> NeedId {
        NeedId(
            self.program
                .task_plans
                .get(plan.index())
                .and_then(|plan| self.program.strings.get(plan.need_id.index()))
                .cloned()
                .unwrap_or_else(|| format!("awbc.need.{}", plan.0)),
        )
    }

    pub(super) fn source_plan_for_id(&self, id: &SourceId) -> Option<AwbcSourcePlanId> {
        self.program
            .source_plans
            .iter()
            .enumerate()
            .find_map(|(index, plan)| {
                self.program
                    .strings
                    .get(plan.public_id.index())
                    .filter(|public_id| public_id.as_str() == id.0)
                    .and_then(|_| u32::try_from(index).ok())
                    .map(AwbcSourcePlanId)
            })
    }
}
