use crate::effect::LineEffectRequest;
use crate::frame::{FrameInput, FrameOutput, RuntimeDiagnostic};
use crate::line_task::run_line_task_group_for_input;
use crate::observation::RuntimeObservationState;
use crate::pattern::{RuntimePattern, match_runtime_pattern};
use crate::plan::{
    ChoiceRuntimeOption, FlowEvent, FlowOp, FlowRuntimeId, RuntimeMatchArm, RuntimeMatchSelection,
    RuntimePlan,
};
use crate::source::{
    SourceEvent, SourceEventKind, SourceHandlerPlan, SourceId, SourceOp, SourcePlan, SourcePolicy,
    SourceRuntimeState, normalize_source_events,
};
use crate::stream::{StreamEvent, StreamMatchArm, StreamOp, StreamRuntimeId, StreamRuntimeState};
use crate::task::{
    AwaitTarget, CancelScopeId, TaskClass, TaskEvent, TaskEventKind, TaskKey, TaskPolicy,
    TaskPriority, TaskSource, TaskSpec, normalize_task_events,
};
use crate::value::{
    RuntimeBinding, RuntimeEnv, RuntimeEvalError, RuntimeExpr, RuntimeExprMatchArm,
    RuntimeFieldValue, RuntimeValue, evaluate_binary, evaluate_unary, expr_runtime_label,
    runtime_value_label,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Engine {
    plan: RuntimePlan,
    fiber: FlowFiber,
}

/// Current flow execution cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowFiber {
    pub line_cursor: usize,
    pub cursor: Option<FlowCursor>,
    pub pending_ops: VecDeque<FlowOp>,
    pub frames: Vec<RuntimeFrame>,
    pub env: RuntimeEnv,
    pub observations: RuntimeObservationState,
    pub source_states: BTreeMap<SourceId, SourceRuntimeState>,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    pub status: FlowFiberStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFrame {
    pub kind: RuntimeFrameKind,
}

/// Structured frame kind for the minimal flow executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeFrameKind {
    Scope,
    Loop {
        body: Vec<FlowOp>,
        result: Option<RuntimePattern>,
    },
    While {
        condition: RuntimeExpr,
        body: Vec<FlowOp>,
    },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
    },
}

/// Position in a lowered flow program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowCursor {
    pub flow: FlowRuntimeId,
    pub op_index: usize,
}

/// High-level flow status for the minimal runtime spine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowFiberStatus {
    Running,
    Waiting(AwaitState),
    Choice(ChoiceState),
    Done(FlowExit),
    Failed(String),
}

/// Suspended `await ... with` state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitState {
    pub target: AwaitTarget,
    pub resume: FlowCursor,
}

/// Suspended choice state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceState {
    pub id: Option<String>,
    pub options: Vec<ChoiceRuntimeOption>,
    pub resume: FlowCursor,
}

/// Terminal flow result observed by the minimal runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowExit {
    Done,
    Return(String),
}

impl Default for FlowFiber {
    fn default() -> Self {
        Self {
            line_cursor: 0,
            cursor: None,
            pending_ops: VecDeque::new(),
            frames: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
            status: FlowFiberStatus::Done(FlowExit::Done),
        }
    }
}

impl FlowCursor {
    fn advanced(&self) -> Self {
        Self {
            flow: self.flow.clone(),
            op_index: self.op_index + 1,
        }
    }
}

impl Default for FlowCursor {
    fn default() -> Self {
        Self {
            flow: FlowRuntimeId(String::new()),
            op_index: 0,
        }
    }
}

impl Engine {
    pub fn new(plan: RuntimePlan) -> Self {
        let cursor = plan.entry_cursor();
        let status = if plan.is_empty() {
            FlowFiberStatus::Done(FlowExit::Done)
        } else {
            FlowFiberStatus::Running
        };
        let source_states = plan
            .source_plans
            .iter()
            .map(|plan| {
                (
                    plan.id.clone(),
                    SourceRuntimeState::new(plan.id.clone(), plan.policy.clone()),
                )
            })
            .collect();
        let stream_states = plan
            .stream_plans
            .iter()
            .map(|plan| (plan.id.clone(), StreamRuntimeState::new(plan.id.clone())))
            .collect();
        Self {
            plan,
            fiber: FlowFiber {
                line_cursor: 0,
                cursor,
                pending_ops: VecDeque::new(),
                frames: Vec::new(),
                env: RuntimeEnv::default(),
                observations: RuntimeObservationState::default(),
                source_states,
                stream_states,
                status,
            },
        }
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.fiber
    }

    pub fn step(&mut self, mut input: FrameInput) -> FrameOutput {
        let mut output = FrameOutput::default();
        self.fiber
            .env
            .bind_all_root(input.external_values.iter().cloned());
        let events = normalize_task_events(std::mem::take(&mut input.task_events));
        let source_events = normalize_source_events(std::mem::take(&mut input.source_events));
        output
            .diagnostics
            .extend(events.iter().map(|event| RuntimeDiagnostic {
                message: format!(
                    "task {} sequence {} delivered",
                    event.task_id.0, event.sequence.0
                ),
            }));
        self.apply_source_events(source_events, &mut output);
        self.step_stream_plans(&mut output);

        if self.resume_suspended(&input, &events, &mut output) {
            self.record_observations(&output.line_effects);
            return output;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            self.record_observations(&output.line_effects);
            return output;
        }
        if self.fiber.cursor.is_some() {
            self.step_flow(&input, &mut output);
        } else {
            self.step_line_only(&input, &mut output);
        }
        self.record_observations(&output.line_effects);
        output
    }

    fn record_observations(&mut self, effects: &[LineEffectRequest]) {
        for effect in effects {
            self.fiber.observations.record_effect(effect);
        }
    }

    fn apply_source_events(
        &mut self,
        events: Vec<SourceEvent<String, String>>,
        output: &mut FrameOutput,
    ) {
        for event in events {
            output.source_events.push(event.clone());
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

    fn dispatch_source_event(
        &mut self,
        plan: &SourcePlan,
        event: SourceEvent<String, String>,
        output: &mut FrameOutput,
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

    fn apply_unhandled_source_event(
        &mut self,
        event: SourceEvent<String, String>,
        output: &mut FrameOutput,
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

    fn record_source_event_state(
        &mut self,
        event: &SourceEvent<String, String>,
        output: &mut FrameOutput,
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

    fn execute_source_ops(
        &mut self,
        source: &SourceId,
        ops: &[SourceOp],
        bindings: Vec<RuntimeBinding>,
        output: &mut FrameOutput,
    ) {
        let previous = self.fiber.env.clone();
        self.fiber.env.push_scope();
        self.fiber.env.bind_all(bindings);
        for op in ops {
            self.execute_source_op(source, op, output);
        }
        self.fiber.env = previous;
    }

    fn execute_source_op(&mut self, source: &SourceId, op: &SourceOp, output: &mut FrameOutput) {
        match op {
            SourceOp::Yield(expr) => match self.evaluate_expr(expr) {
                Ok(value) => self.push_source_item(source, runtime_value_label(&value), output),
                Err(error) => Self::diagnose_runtime_error(error, output),
            },
            SourceOp::Effect(effect) => output.line_effects.push(effect.clone()),
            SourceOp::SignalWrite(write) => output
                .line_effects
                .push(LineEffectRequest::SignalWrite(write.clone())),
            SourceOp::Log(log) => output
                .line_effects
                .push(LineEffectRequest::Log(log.clone())),
            SourceOp::Close(target) => self.close_source(target, output),
            SourceOp::Noop => {}
        }
    }

    fn push_source_item(&mut self, source: &SourceId, item: String, output: &mut FrameOutput) {
        let state = self
            .fiber
            .source_states
            .entry(source.clone())
            .or_insert_with(|| SourceRuntimeState::new(source.clone(), SourcePolicy::default()));
        if let Some(message) = state.push_item(item) {
            output.diagnostics.push(RuntimeDiagnostic { message });
        }
    }

    fn close_source(&mut self, source: &SourceId, output: &mut FrameOutput) {
        if let Some(state) = self.fiber.source_states.get_mut(source) {
            state.close();
        }
        output.source_close_requests.push(source.clone());
    }

    fn step_stream_plans(&mut self, output: &mut FrameOutput) {
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

    fn execute_stream_ops(
        &mut self,
        stream: &StreamRuntimeId,
        ops: &[StreamOp],
        budget: &mut usize,
        output: &mut FrameOutput,
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

    fn execute_stream_op(
        &mut self,
        stream: &StreamRuntimeId,
        op: &StreamOp,
        budget: &mut usize,
        output: &mut FrameOutput,
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

    fn bind_stream_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut FrameOutput,
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

    fn execute_stream_for_next(
        &mut self,
        stream: &StreamRuntimeId,
        pattern: &RuntimePattern,
        source: &RuntimeExpr,
        body: &[StreamOp],
        budget: &mut usize,
        output: &mut FrameOutput,
    ) -> bool {
        let Ok(source_key) = self.evaluate_queue_target(source) else {
            return true;
        };
        while let Some(item) = self.pop_queue_item(&source_key) {
            let previous = self.fiber.env.clone();
            self.fiber.env.push_scope();
            match match_runtime_pattern(pattern, &RuntimeValue::String(item)) {
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

    fn yield_stream_item(
        &mut self,
        stream: &StreamRuntimeId,
        expr: &RuntimeExpr,
        output: &mut FrameOutput,
    ) -> bool {
        match self.evaluate_expr(expr) {
            Ok(value) => {
                let item = runtime_value_label(&value);
                let state = self
                    .fiber
                    .stream_states
                    .entry(stream.clone())
                    .or_insert_with(|| StreamRuntimeState::new(stream.clone()));
                let sequence = state.push_item(item.clone());
                output.stream_events.push(StreamEvent {
                    stream: stream.clone(),
                    sequence,
                    kind: SourceEventKind::Item(item),
                });
            }
            Err(error) => Self::diagnose_runtime_error(error, output),
        }
        true
    }

    fn execute_stream_match(
        &mut self,
        stream: &StreamRuntimeId,
        scrutinee: &RuntimeExpr,
        arms: &[StreamMatchArm],
        budget: &mut usize,
        output: &mut FrameOutput,
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

    fn close_stream_source(&mut self, source: &RuntimeExpr, output: &mut FrameOutput) {
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

    fn evaluate_queue_target(&mut self, expr: &RuntimeExpr) -> Result<String, RuntimeEvalError> {
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

    fn pop_queue_item(&mut self, key: &str) -> Option<String> {
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

    fn diagnose_runtime_error(error: impl std::fmt::Display, output: &mut FrameOutput) {
        output.diagnostics.push(RuntimeDiagnostic {
            message: error.to_string(),
        });
    }

    fn resume_suspended(
        &mut self,
        input: &FrameInput,
        events: &[TaskEvent],
        output: &mut FrameOutput,
    ) -> bool {
        match self.fiber.status.clone() {
            FlowFiberStatus::Waiting(state) => {
                self.resume_await_state(state, events, output);
                true
            }
            FlowFiberStatus::Choice(state) => {
                self.resume_choice_state(state, input, output);
                true
            }
            FlowFiberStatus::Running | FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_) => {
                false
            }
        }
    }

    fn resume_await_state(
        &mut self,
        state: AwaitState,
        events: &[TaskEvent],
        output: &mut FrameOutput,
    ) {
        let Some(event) = events
            .iter()
            .find(|event| event.task_id == state.target.task)
            .cloned()
        else {
            self.fiber.status = FlowFiberStatus::Waiting(state);
            return;
        };
        match event.kind {
            TaskEventKind::Ready(value) => {
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: state.target.need,
                    value,
                });
                self.fiber.cursor = Some(state.resume);
                self.fiber.status = FlowFiberStatus::Running;
            }
            TaskEventKind::Progress(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: state.target.need.clone(),
                    progress,
                });
                self.fiber.status = FlowFiberStatus::Waiting(state);
            }
            TaskEventKind::Err(error) => {
                self.fiber.status = FlowFiberStatus::Failed(error.clone());
                output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("await task {} failed: {error}", state.target.task.0),
                });
            }
            TaskEventKind::Cancelled => {
                let message = format!("await task {} was cancelled", state.target.task.0);
                self.fiber.status = FlowFiberStatus::Failed(message.clone());
                output.diagnostics.push(RuntimeDiagnostic { message });
            }
        }
    }

    fn resume_choice_state(
        &mut self,
        state: ChoiceState,
        input: &FrameInput,
        output: &mut FrameOutput,
    ) {
        let Some(option) = state
            .options
            .iter()
            .find(|option| input_selects_choice(input, option))
            .cloned()
        else {
            self.fiber.status = FlowFiberStatus::Choice(state);
            return;
        };
        let selected = option.id.clone().unwrap_or_else(|| option.label.clone());
        output.flow_events.push(FlowEvent::ChoiceSelected {
            id: state.id.clone(),
            option: selected,
        });
        output.line_effects.extend(option.effects.clone());
        if let Some(out) = option.out {
            output.line_effects.push(LineEffectRequest::Out(out));
        }
        if let Some(target) = option.target {
            self.goto(target, output);
        } else {
            self.fiber.cursor = Some(state.resume);
            self.fiber.status = FlowFiberStatus::Running;
        }
    }

    // Keep the opcode dispatcher contiguous while the Phase 1 runtime surface is
    // still changing; extracting each arm now would obscure grammar coverage.
    #[allow(clippy::too_many_lines)]
    fn step_flow(&mut self, input: &FrameInput, output: &mut FrameOutput) {
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

    fn bind_value(
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

    fn advance_if_needed(&mut self, next: Option<FlowCursor>) {
        if let Some(next) = next {
            self.fiber.cursor = Some(next);
        }
    }

    fn push_ops(&mut self, ops: Vec<FlowOp>) {
        for op in ops.into_iter().rev() {
            self.fiber.pending_ops.push_front(op);
        }
    }

    fn scoped_ops(mut ops: Vec<FlowOp>) -> Vec<FlowOp> {
        if ops.is_empty() {
            return Vec::new();
        }
        ops.insert(0, FlowOp::EnterScope);
        ops.push(FlowOp::ExitScope);
        ops
    }

    fn push_scoped_ops(&mut self, ops: Vec<FlowOp>) {
        self.push_ops(Self::scoped_ops(ops));
    }

    fn push_scoped_ops_with_bindings(
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

    fn push_loop_iteration(&mut self, body: Vec<FlowOp>) {
        let mut ops = Self::scoped_ops(body.clone());
        ops.push(FlowOp::LoopNext { body });
        self.push_ops(ops);
    }

    fn push_while_iteration(&mut self, condition: RuntimeExpr, body: Vec<FlowOp>) {
        let mut ops = Self::scoped_ops(body.clone());
        ops.push(FlowOp::WhileNext { condition, body });
        self.push_ops(ops);
    }

    fn push_while_let_iteration(
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

    fn pop_scope_frame(&mut self) {
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

    fn pop_scope_frames_until_loop(&mut self) {
        while matches!(
            self.fiber.frames.last(),
            Some(RuntimeFrame {
                kind: RuntimeFrameKind::Scope
            })
        ) {
            self.pop_scope_frame();
        }
    }

    fn pop_loop_frame(&mut self) -> Option<RuntimeFrameKind> {
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

    fn discard_pending_until_loop_next(&mut self) {
        while let Some(op) = self.fiber.pending_ops.pop_front() {
            if matches!(
                op,
                FlowOp::LoopNext { .. } | FlowOp::WhileNext { .. } | FlowOp::WhileLetNext { .. }
            ) {
                break;
            }
        }
    }

    fn break_nearest_loop(&mut self, value: &RuntimeValue, output: &mut FrameOutput) -> bool {
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

    fn continue_nearest_loop(&mut self, output: &mut FrameOutput) -> bool {
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

    fn evaluate_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut FrameOutput,
    ) {
        match self.evaluate_expr(expr).and_then(|value| {
            self.try_bind_pattern(pattern, &value)
                .map(|matched| (matched, value))
        }) {
            Ok((true, _)) => {}
            Ok((false, value)) => {
                self.fail_eval(
                    RuntimeEvalError::PatternMismatch(runtime_value_label(&value)),
                    output,
                );
            }
            Err(error) => self.fail_eval(error, output),
        }
    }

    fn evaluate_if_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
    ) -> Result<Option<Vec<RuntimeBinding>>, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        let Some(bindings) = match_runtime_pattern(pattern, &value)? else {
            return Ok(None);
        };
        if let Some(guard) = guard {
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings.clone());
            let matched = self.evaluate_bool(guard);
            self.fiber.env = previous;
            let matched = matched?;
            if !matched {
                return Ok(None);
            }
        }
        Ok(Some(bindings))
    }

    fn evaluate_match(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeMatchArm],
    ) -> Result<RuntimeMatchSelection, RuntimeEvalError> {
        let value = self.evaluate_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings.clone());
            if let Some(guard) = arm.guard.as_ref()
                && !match self.evaluate_bool(guard) {
                    Ok(matched) => matched,
                    Err(error) => {
                        self.fiber.env = previous;
                        return Err(error);
                    }
                }
            {
                self.fiber.env = previous;
                continue;
            }
            self.fiber.env = previous;
            return Ok(Some((bindings, arm.ops.clone())));
        }
        Ok(None)
    }

    fn evaluate_expr(&mut self, expr: &RuntimeExpr) -> Result<RuntimeValue, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(value) => Ok(value.clone()),
            RuntimeExpr::Local(name) => self
                .fiber
                .env
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeEvalError::UnknownBinding(name.clone())),
            RuntimeExpr::EntityRef(target) => Ok(RuntimeValue::EntityRef(target.clone())),
            RuntimeExpr::Tuple(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Tuple),
            RuntimeExpr::BracketSeq(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::BracketSeq),
            RuntimeExpr::Record(fields) => fields
                .iter()
                .map(|field| {
                    Ok(RuntimeFieldValue {
                        name: field.name.clone(),
                        value: self.evaluate_expr(&field.value)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Record),
            RuntimeExpr::Variant {
                path,
                name,
                payload,
            } => Ok(RuntimeValue::Variant {
                path: path.clone(),
                name: name.clone(),
                payload: payload
                    .as_ref()
                    .map(|expr| self.evaluate_expr(expr).map(Box::new))
                    .transpose()?,
            }),
            RuntimeExpr::Field { target, field } => {
                let value = self.evaluate_expr(target)?;
                match value {
                    RuntimeValue::Record(fields) => fields
                        .into_iter()
                        .find(|candidate| candidate.name == *field)
                        .map(|field| field.value)
                        .ok_or_else(|| RuntimeEvalError::MissingField {
                            field: field.clone(),
                            value: "record".to_owned(),
                        }),
                    value => Err(RuntimeEvalError::MissingField {
                        field: field.clone(),
                        value: runtime_value_label(&value),
                    }),
                }
            }
            RuntimeExpr::Unary { op, expr } => {
                let value = self.evaluate_expr(expr)?;
                evaluate_unary(*op, value)
            }
            RuntimeExpr::Binary { lhs, op, rhs } => {
                let lhs = self.evaluate_expr(lhs)?;
                let rhs = self.evaluate_expr(rhs)?;
                evaluate_binary(lhs, *op, rhs)
            }
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.evaluate_bool(condition)? {
                    self.evaluate_expr(then_expr)
                } else {
                    self.evaluate_expr(else_expr)
                }
            }
            RuntimeExpr::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => self.evaluate_if_let_expr(pattern, expr, guard.as_deref(), then_expr, else_expr),
            RuntimeExpr::Match { scrutinee, arms } => self.evaluate_match_expr(scrutinee, arms),
        }
    }

    fn evaluate_if_let_expr(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        let Some(bindings) = match_runtime_pattern(pattern, &value)? else {
            return self.evaluate_expr(else_expr);
        };
        let previous = self.fiber.env.clone();
        self.fiber.env.bind_all(bindings);
        match guard.map_or(Ok(true), |guard| self.evaluate_bool(guard)) {
            Ok(true) => {
                let result = self.evaluate_expr(then_expr);
                self.fiber.env = previous;
                result
            }
            Ok(false) => {
                self.fiber.env = previous;
                self.evaluate_expr(else_expr)
            }
            Err(error) => {
                self.fiber.env = previous;
                Err(error)
            }
        }
    }

    fn evaluate_match_expr(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeExprMatchArm],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings);
            if let Some(guard) = arm.guard.as_ref()
                && !match self.evaluate_bool(guard) {
                    Ok(matched) => matched,
                    Err(error) => {
                        self.fiber.env = previous;
                        return Err(error);
                    }
                }
            {
                self.fiber.env = previous;
                continue;
            }
            let result = self.evaluate_expr(&arm.value);
            self.fiber.env = previous;
            return result;
        }
        Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
            &value,
        )))
    }

    fn evaluate_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(runtime_value_label(&value))),
        }
    }

    fn evaluate_entity_target(&mut self, expr: &RuntimeExpr) -> Result<String, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::EntityRef(target) | RuntimeValue::String(target) => Ok(target),
            value => Err(RuntimeEvalError::ExpectedEntityRef(runtime_value_label(
                &value,
            ))),
        }
    }

    fn try_bind_pattern(
        &mut self,
        pattern: &RuntimePattern,
        value: &RuntimeValue,
    ) -> Result<bool, RuntimeEvalError> {
        let Some(bindings) = match_runtime_pattern(pattern, value)? else {
            return Ok(false);
        };
        self.fiber.env.bind_all(bindings);
        Ok(true)
    }

    fn fail_eval(&mut self, error: impl std::fmt::Display, output: &mut FrameOutput) {
        let message = error.to_string();
        self.fiber.status = FlowFiberStatus::Failed(message.clone());
        output.diagnostics.push(RuntimeDiagnostic { message });
    }

    fn step_line_only(&mut self, input: &FrameInput, output: &mut FrameOutput) {
        let Some(group) = self.plan.line_task_groups.get(self.fiber.line_cursor) else {
            self.finish(output);
            return;
        };
        output.merge(run_line_task_group_for_input(group, input));
        self.fiber.line_cursor += 1;
        if self.fiber.line_cursor >= self.plan.line_task_groups.len() {
            self.finish(output);
        }
    }

    fn apply_control_effects(&mut self, output: &mut FrameOutput) -> bool {
        let Some(control) = output.line_effects.iter().find_map(control_from_effect) else {
            return false;
        };
        match control {
            FlowControl::Goto(target) => self.goto(FlowRuntimeId(target), output),
            FlowControl::Return(value) => self.return_value(value, output),
            FlowControl::Failed(message) => self.fiber.status = FlowFiberStatus::Failed(message),
        }
        true
    }

    fn goto(&mut self, target: FlowRuntimeId, output: &mut FrameOutput) {
        self.fiber.pending_ops.clear();
        output.flow_events.push(FlowEvent::Goto {
            target: target.clone(),
        });
        self.fiber.cursor = Some(FlowCursor {
            flow: target,
            op_index: 0,
        });
        self.fiber.status = FlowFiberStatus::Running;
    }

    fn return_value(&mut self, value: String, output: &mut FrameOutput) {
        self.fiber.pending_ops.clear();
        output.flow_events.push(FlowEvent::Return {
            value: value.clone(),
        });
        self.fiber.status = FlowFiberStatus::Done(FlowExit::Return(value));
    }

    fn finish(&mut self, output: &mut FrameOutput) {
        output.flow_events.push(FlowEvent::Done);
        self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
    }
}

enum FlowControl {
    Goto(String),
    Return(String),
    Failed(String),
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

fn control_from_effect(effect: &LineEffectRequest) -> Option<FlowControl> {
    match effect {
        LineEffectRequest::Goto(target) => Some(FlowControl::Goto(target.clone())),
        LineEffectRequest::Return(value) => Some(FlowControl::Return(value.clone())),
        LineEffectRequest::Panic(message)
        | LineEffectRequest::Fail(message)
        | LineEffectRequest::Bail(message) => Some(FlowControl::Failed(message.clone())),
        _ => None,
    }
}

fn input_selects_choice(input: &FrameInput, option: &ChoiceRuntimeOption) -> bool {
    input.input_events.iter().any(|event| {
        let Some(payload) = event.payload.as_deref() else {
            return false;
        };
        matches!(event.kind.as_str(), "choice" | "select")
            && (option.id.as_deref() == Some(payload) || option.label == payload)
    })
}

fn await_task_spec(target: &AwaitTarget) -> TaskSpec {
    TaskSpec {
        id: target.task.clone(),
        key: TaskKey(target.task.0.clone()),
        class: TaskClass::Background,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("flow".to_owned()),
        policy: TaskPolicy::JoinSameKey,
        source: TaskSource {
            label: format!("await {}", target.need.0),
        },
    }
}
