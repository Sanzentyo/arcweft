use crate::effect::LineEffectRequest;
use crate::line_task::run_line_task_group_for_input;
use crate::observation::RuntimeObservationState;
use crate::pattern::{RuntimePattern, match_runtime_pattern};
use crate::plan::{
    ChoiceRuntimeOption, FlowEvent, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeMatchArm,
    RuntimeMatchSelection, RuntimePlan,
};
use crate::pure::{RuntimePureCallBackend, VmRuntimePureCallBackend};
use crate::source::{
    RuntimeSourceEvent, SourceEventKind, SourceHandlerPlan, SourceId, SourceOp, SourcePlan,
    SourcePolicy, SourceRuntimeState, normalize_source_events,
};
use crate::step::{
    RuntimeDiagnostic, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepOutput,
    RuntimeStepResult, RuntimeStepStats, RuntimeStepStopReason,
};
use crate::stream::{
    RuntimeStreamEvent, StreamMatchArm, StreamOp, StreamRuntimeId, StreamRuntimeState,
};
use crate::task::{
    AwaitManyTarget, AwaitTarget, CancelScopeId, NeedId, TaskEvent, TaskEventKind, TaskId, TaskKey,
    TaskPolicy, TaskPriority, TaskSpec, normalize_task_events,
};
use crate::value::{
    RuntimeBinding, RuntimeEnv, RuntimeEvalError, RuntimeExpr, RuntimeExprMatchArm,
    RuntimeFieldValue, RuntimePayload, RuntimeValue, evaluate_binary, evaluate_unary,
    expr_runtime_label, runtime_value_label,
};
use std::collections::{BTreeMap, VecDeque};
pub mod aot;
pub mod eval;
pub mod flow;
pub mod line;
pub mod source;
pub mod stream;
pub mod suspend;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Engine {
    plan: RuntimePlan,
    flow_positions: BTreeMap<FlowRuntimeId, usize>,
    fiber: FlowFiber,
    child_fibers: VecDeque<FlowFiber>,
    run_child_next: bool,
    pure_i64_batch_inputs: Vec<i64>,
    pure_i64_batch_outputs: Vec<i64>,
    pure_helper_i64_results: Vec<bool>,
}

/// Current flow execution cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowFiber {
    pub line_cursor: usize,
    pub cursor: Option<FlowCursor>,
    pub pending_ops: VecDeque<FlowOp>,
    pub control_stack: Vec<FlowControlStackEntry>,
    pub env: RuntimeEnv,
    pub observations: RuntimeObservationState,
    pub source_states: BTreeMap<SourceId, SourceRuntimeState>,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    pub status: FlowFiberStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowControlStackEntry {
    pub kind: FlowControlStackEntryKind,
}

/// Structured frame kind for the minimal flow executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowControlStackEntryKind {
    Scope,
    Loop {
        body: std::sync::Arc<[FlowOp]>,
        result: Option<RuntimePattern>,
    },
    While {
        condition: RuntimeExpr,
        body: std::sync::Arc<[FlowOp]>,
    },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: std::sync::Arc<[FlowOp]>,
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
    WaitingMany(Box<AwaitManyState>),
    Choice(ChoiceState),
    Done(FlowExit),
    Failed(String),
}

/// Suspended `await ... with` state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitState {
    pub binding: Option<RuntimePattern>,
    pub target: AwaitTarget,
    pub resume: FlowCursor,
}

/// Suspended bounded fanout await state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitManyState {
    pub binding: Option<RuntimePattern>,
    pub target: AwaitManyTarget,
    pub resume: FlowCursor,
    pub items: Vec<RuntimeValue>,
    pub next_index: usize,
    pub in_flight: Vec<AwaitManyInFlight>,
    pub results: Vec<Option<String>>,
}

/// One in-flight child task inside a bounded fanout await.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitManyInFlight {
    pub index: usize,
    pub task: TaskId,
    pub need: NeedId,
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
            control_stack: Vec::new(),
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
        let flow_positions = plan
            .flows
            .iter()
            .enumerate()
            .map(|(index, flow)| (flow.id.clone(), index))
            .collect();
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
        let pure_helper_i64_results = plan
            .pure_helpers
            .iter()
            .map(eval::pure_helper_returns_i64)
            .collect();
        Self {
            plan,
            flow_positions,
            fiber: FlowFiber {
                line_cursor: 0,
                cursor,
                pending_ops: VecDeque::new(),
                control_stack: Vec::new(),
                env: RuntimeEnv::default(),
                observations: RuntimeObservationState::default(),
                source_states,
                stream_states,
                status,
            },
            child_fibers: VecDeque::new(),
            run_child_next: false,
            pure_i64_batch_inputs: Vec::new(),
            pure_i64_batch_outputs: Vec::new(),
            pure_helper_i64_results,
        }
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.fiber
    }

    pub(super) fn flow_at_cursor(&self, cursor: &FlowCursor) -> Option<&RuntimeFlow> {
        self.flow_positions
            .get(&cursor.flow)
            .and_then(|index| self.plan.flows.get(*index))
    }

    pub fn child_fiber_count(&self) -> usize {
        self.child_fibers.len()
    }

    pub fn step(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        let mut pure_backend = VmRuntimePureCallBackend::default();
        self.step_with_pure_backend(input, options, &mut pure_backend)
    }

    pub fn step_with_pure_backend(
        &mut self,
        mut input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> RuntimeStepResult {
        let mut output = RuntimeStepOutput::default();
        let mut executed_ops = 0;
        let pure_stats_before = pure_backend.stats();
        let pending_ops_before = self.pending_ops_len();
        self.fiber
            .env
            .bind_all_root(std::mem::take(&mut input.bindings));
        let events = normalize_task_events(std::mem::take(&mut input.task_events));
        let source_events = normalize_source_events(std::mem::take(&mut input.source_events));
        let task_events_in = events.len();
        let source_events_in = source_events.len();
        output
            .diagnostics
            .extend(events.iter().map(|event| RuntimeDiagnostic {
                message: format!(
                    "task {} sequence {} delivered",
                    event.task_id.0, event.sequence.0
                ),
            }));
        self.apply_source_events(source_events, &mut output, pure_backend);
        self.step_stream_plans(&mut output, pure_backend);

        while executed_ops < options.budget.max_ops && self.can_attempt_runtime_op() {
            self.step_runtime_op(&input, &events, &mut output, pure_backend);
            executed_ops += 1;
            if self.should_return_to_host(options.mode, &output, executed_ops) {
                break;
            }
        }
        self.record_observations(&output.effects.line);
        let stats = RuntimeStepStats {
            executed_ops,
            pending_ops_before,
            pending_ops_after: self.pending_ops_len(),
            child_fibers: self.child_fibers.len(),
            pure: pure_backend.stats().saturating_delta(pure_stats_before),
            task_events_in,
            source_events_in,
            source_events_emitted: output.effects.source_events.len(),
            stream_events_emitted: output.effects.stream_events.len(),
            line_effects: output.effects.line.len(),
            diagnostics: output.diagnostics.len(),
        };
        self.step_result(output, options, stats)
    }

    fn can_attempt_runtime_op(&self) -> bool {
        self.main_fiber_can_attempt_runtime_op() || self.has_active_child_fibers()
    }

    fn step_runtime_op(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) {
        if self.run_child_next && self.step_next_child_fiber(input, events, output, pure_backend) {
            self.run_child_next = false;
            return;
        }
        self.run_child_next = true;
        if !self.main_fiber_can_attempt_runtime_op() {
            self.step_next_child_fiber(input, events, output, pure_backend);
            return;
        }
        if self.resume_suspended(input, events, output, pure_backend) {
            return;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return;
        }
        if self.fiber.cursor.is_some() {
            self.step_flow(input, output, pure_backend);
        } else {
            self.step_line_only(input, output);
        }
    }

    fn main_fiber_can_attempt_runtime_op(&self) -> bool {
        !matches!(
            self.fiber.status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        )
    }

    pub(super) fn has_active_child_fibers(&self) -> bool {
        !self.child_fibers.is_empty()
    }

    fn pending_ops_len(&self) -> usize {
        self.fiber.pending_ops.len()
            + self
                .child_fibers
                .iter()
                .map(|fiber| fiber.pending_ops.len())
                .sum::<usize>()
    }

    pub(super) fn spawn_child_fiber(&mut self, body: Vec<FlowOp>) {
        let mut pending_ops = VecDeque::new();
        for op in Self::scoped_ops(body).into_iter().rev() {
            pending_ops.push_front(op);
        }
        self.child_fibers.push_back(FlowFiber {
            line_cursor: 0,
            cursor: None,
            pending_ops,
            control_stack: Vec::new(),
            env: self.fiber.env.clone(),
            observations: RuntimeObservationState::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
            status: FlowFiberStatus::Running,
        });
        self.run_child_next = true;
    }

    fn step_next_child_fiber(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> bool {
        let Some(mut child) = self.child_fibers.pop_front() else {
            return false;
        };
        std::mem::swap(&mut self.fiber, &mut child);
        self.step_active_child_fiber(input, events, output, pure_backend);
        self.finish_active_child_if_exhausted();
        std::mem::swap(&mut self.fiber, &mut child);
        match child.status {
            FlowFiberStatus::Done(_) => {}
            FlowFiberStatus::Failed(message) => {
                self.fiber.status = FlowFiberStatus::Failed(message);
            }
            _ => self.child_fibers.push_back(child),
        }
        true
    }

    fn step_active_child_fiber(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimePureCallBackend,
    ) {
        if self.resume_suspended(input, events, output, pure_backend) {
            return;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return;
        }
        if self.fiber.cursor.is_some() || !self.fiber.pending_ops.is_empty() {
            self.step_flow(input, output, pure_backend);
        }
    }

    fn finish_active_child_if_exhausted(&mut self) {
        if matches!(self.fiber.status, FlowFiberStatus::Running)
            && self.fiber.cursor.is_none()
            && self.fiber.pending_ops.is_empty()
        {
            self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
        }
    }

    fn should_return_to_host(
        &self,
        mode: RuntimeStepMode,
        output: &RuntimeStepOutput,
        executed_ops: usize,
    ) -> bool {
        if self.hard_stop_reason(output).is_some() {
            if matches!(mode, RuntimeStepMode::Drain | RuntimeStepMode::Server)
                && has_host_requests(output)
                && (self.main_fiber_can_attempt_runtime_op() || self.has_runnable_child_fibers())
            {
                return false;
            }
            return true;
        }
        match mode {
            RuntimeStepMode::OneOp => executed_ops > 0,
            RuntimeStepMode::Game => has_presentation_visible_output(output),
            RuntimeStepMode::Drain | RuntimeStepMode::Server => false,
        }
    }

    fn record_observations(&mut self, effects: &[LineEffectRequest]) {
        for effect in effects {
            self.fiber.observations.record_effect(effect);
        }
    }

    fn diagnose_runtime_error(error: impl std::fmt::Display, output: &mut RuntimeStepOutput) {
        output.diagnostics.push(RuntimeDiagnostic {
            message: error.to_string(),
        });
    }

    fn step_result(
        &self,
        output: RuntimeStepOutput,
        options: RuntimeStepOptions,
        stats: RuntimeStepStats,
    ) -> RuntimeStepResult {
        let stop_reason = self
            .hard_stop_reason(&output)
            .unwrap_or_else(|| Self::running_stop_reason(options, stats.executed_ops, &output));
        RuntimeStepResult {
            output,
            fiber_status: self.effective_fiber_status(),
            stop_reason,
            stats,
        }
    }

    fn effective_fiber_status(&self) -> FlowFiberStatus {
        if self.has_active_child_fibers()
            && matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_)
                    | FlowFiberStatus::Waiting(_)
                    | FlowFiberStatus::WaitingMany(_)
                    | FlowFiberStatus::Choice(_)
            )
        {
            FlowFiberStatus::Running
        } else {
            self.fiber.status.clone()
        }
    }

    fn hard_stop_reason(&self, output: &RuntimeStepOutput) -> Option<RuntimeStepStopReason> {
        if self.has_active_child_fibers()
            && matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_)
                    | FlowFiberStatus::Waiting(_)
                    | FlowFiberStatus::WaitingMany(_)
                    | FlowFiberStatus::Choice(_)
            )
        {
            return None;
        }
        match self.fiber.status {
            FlowFiberStatus::Done(_) => Some(RuntimeStepStopReason::Done),
            FlowFiberStatus::Failed(_) => Some(RuntimeStepStopReason::Failed),
            FlowFiberStatus::Waiting(_)
            | FlowFiberStatus::WaitingMany(_)
            | FlowFiberStatus::Choice(_) => Some(RuntimeStepStopReason::Blocked),
            FlowFiberStatus::Running if has_host_requests(output) => {
                Some(RuntimeStepStopReason::Output)
            }
            FlowFiberStatus::Running => None,
        }
    }

    fn running_stop_reason(
        options: RuntimeStepOptions,
        executed_ops: usize,
        output: &RuntimeStepOutput,
    ) -> RuntimeStepStopReason {
        if options.mode == RuntimeStepMode::Game && has_presentation_visible_output(output) {
            return RuntimeStepStopReason::Output;
        }
        if options.mode == RuntimeStepMode::OneOp && executed_ops > 0 {
            return RuntimeStepStopReason::OneOp;
        }
        if executed_ops >= options.budget.max_ops {
            return RuntimeStepStopReason::BudgetExhausted;
        }
        RuntimeStepStopReason::OneOp
    }

    fn has_runnable_child_fibers(&self) -> bool {
        self.child_fibers.iter().any(|child| {
            matches!(child.status, FlowFiberStatus::Running)
                && (child.cursor.is_some() || !child.pending_ops.is_empty())
        })
    }
}

fn has_host_requests(output: &RuntimeStepOutput) -> bool {
    !output.requests.tasks.is_empty()
        || !output.requests.cancel_scopes.is_empty()
        || !output.requests.source_close.is_empty()
}

fn has_presentation_visible_output(output: &RuntimeStepOutput) -> bool {
    output.flow_events.iter().any(flow_event_is_visible)
        || output.effects.line.iter().any(line_effect_is_visible)
}

fn flow_event_is_visible(event: &FlowEvent) -> bool {
    matches!(
        event,
        FlowEvent::DialogueLine { .. }
            | FlowEvent::LineCancelled { .. }
            | FlowEvent::ChoicePresented { .. }
            | FlowEvent::ChoiceSelected { .. }
            | FlowEvent::AwaitStarted { .. }
            | FlowEvent::AwaitProgress { .. }
    )
}

fn line_effect_is_visible(effect: &LineEffectRequest) -> bool {
    !matches!(
        effect,
        LineEffectRequest::Log(_)
            | LineEffectRequest::SignalWrite(_)
            | LineEffectRequest::MetricWrite(_)
            | LineEffectRequest::EmitEvent(_)
    )
}
