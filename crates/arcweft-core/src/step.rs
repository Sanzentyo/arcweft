use crate::effect::LineEffectRequest;
use crate::plan::FlowEvent;
use crate::source::{RuntimeSourceEvent, SourceId};
use crate::stream::RuntimeStreamEvent;
use crate::task::{CancelScopeId, TaskEvent, TaskSpec};
use crate::time::{LogicalDuration, TickId};
use crate::value::RuntimeBinding;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStepInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub bindings: Vec<RuntimeBinding>,
    pub input_events: Vec<InputEvent>,
    pub task_events: Vec<TaskEvent>,
    pub ui_events: Vec<UiEvent>,
    pub audio_events: Vec<AudioEvent>,
    pub source_events: Vec<RuntimeSourceEvent>,
}

/// Borrowed adapter-facing view of runtime step inputs.
///
/// Adapters should prefer this view when handing input data into lower runtime
/// layers. The view keeps ownership at the adapter step boundary and makes it
/// clear that runtime code must not retain borrowed event slices past the step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStepInputRef<'a> {
    tick: TickId,
    dt: LogicalDuration,
    bindings: &'a [RuntimeBinding],
    input_events: &'a [InputEvent],
    task_events: &'a [TaskEvent],
    ui_events: &'a [UiEvent],
    audio_events: &'a [AudioEvent],
    source_events: &'a [RuntimeSourceEvent],
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStepOutput {
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub flow_events: Vec<FlowEvent>,
    pub effects: RuntimeEffectBatch,
    pub requests: HostRequestBatch,
}

/// Runtime-produced events and effect requests, kept as pure data for hosts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeEffectBatch {
    pub line: Vec<LineEffectRequest>,
    pub source_events: Vec<RuntimeSourceEvent>,
    pub stream_events: Vec<RuntimeStreamEvent>,
}

/// Host-facing requests emitted by one deterministic runtime step.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostRequestBatch {
    pub tasks: Vec<TaskSpec>,
    pub cancel_scopes: Vec<CancelScopeId>,
    pub source_close: Vec<SourceId>,
}

/// Result envelope returned by the runtime step boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStepResult {
    pub output: RuntimeStepOutput,
    pub fiber_status: crate::engine::FlowFiberStatus,
    pub stop_reason: RuntimeStepStopReason,
    pub stats: RuntimeStepStats,
}

/// Deterministic counters for one runtime step.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStepStats {
    pub executed_ops: usize,
    pub pending_ops_before: usize,
    pub pending_ops_after: usize,
    pub child_fibers: usize,
    pub pure: RuntimePureCallStats,
    pub task_events_in: usize,
    pub source_events_in: usize,
    pub source_events_emitted: usize,
    pub stream_events_emitted: usize,
    pub line_effects: usize,
    pub diagnostics: usize,
}

/// Deterministic counters for runtime pure helper acceleration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimePureCallStats {
    pub pure_calls: usize,
    pub batch_calls: usize,
    pub batch_items: usize,
    pub flat_batch_calls: usize,
    pub flat_batch_items: usize,
    pub flat_batch_bytes_borrowed: usize,
    pub flatten_materializations: usize,
    pub flatten_bytes_copied: usize,
    pub jit_calls: usize,
    pub aot_calls: usize,
    pub vm_calls: usize,
    pub arg_stack_packs: usize,
    pub arg_vec_allocations: usize,
    pub arg_bytes_copied: usize,
    pub arg_bytes_borrowed: usize,
    pub result_bytes_copied: usize,
    pub thread_pool_jobs: usize,
    pub fallbacks: usize,
}

impl RuntimePureCallStats {
    #[must_use]
    pub fn saturating_delta(self, before: Self) -> Self {
        Self {
            pure_calls: self.pure_calls.saturating_sub(before.pure_calls),
            batch_calls: self.batch_calls.saturating_sub(before.batch_calls),
            batch_items: self.batch_items.saturating_sub(before.batch_items),
            flat_batch_calls: self
                .flat_batch_calls
                .saturating_sub(before.flat_batch_calls),
            flat_batch_items: self
                .flat_batch_items
                .saturating_sub(before.flat_batch_items),
            flat_batch_bytes_borrowed: self
                .flat_batch_bytes_borrowed
                .saturating_sub(before.flat_batch_bytes_borrowed),
            flatten_materializations: self
                .flatten_materializations
                .saturating_sub(before.flatten_materializations),
            flatten_bytes_copied: self
                .flatten_bytes_copied
                .saturating_sub(before.flatten_bytes_copied),
            jit_calls: self.jit_calls.saturating_sub(before.jit_calls),
            aot_calls: self.aot_calls.saturating_sub(before.aot_calls),
            vm_calls: self.vm_calls.saturating_sub(before.vm_calls),
            arg_stack_packs: self.arg_stack_packs.saturating_sub(before.arg_stack_packs),
            arg_vec_allocations: self
                .arg_vec_allocations
                .saturating_sub(before.arg_vec_allocations),
            arg_bytes_copied: self
                .arg_bytes_copied
                .saturating_sub(before.arg_bytes_copied),
            arg_bytes_borrowed: self
                .arg_bytes_borrowed
                .saturating_sub(before.arg_bytes_borrowed),
            result_bytes_copied: self
                .result_bytes_copied
                .saturating_sub(before.result_bytes_copied),
            thread_pool_jobs: self
                .thread_pool_jobs
                .saturating_sub(before.thread_pool_jobs),
            fallbacks: self.fallbacks.saturating_sub(before.fallbacks),
        }
    }
}

/// Runtime stepping policy selected by hosts and CLI tooling.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStepOptions {
    pub mode: RuntimeStepMode,
    pub budget: RuntimeStepBudget,
}

/// Deterministic work budget for one `Engine::step` call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStepBudget {
    pub max_ops: usize,
}

/// Runtime drain strategy. Adapter loops remain outside `arcweft-core`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeStepMode {
    #[default]
    OneOp,
    Drain,
    Game,
    Server,
}

/// Reason the runtime stopped returning control to the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeStepStopReason {
    OneOp,
    Blocked,
    Output,
    BudgetExhausted,
    Done,
    Failed,
}

/// Mutable adapter-facing writer for runtime step outputs.
///
/// The writer gives adapter/runtime integration code a scoped output sink
/// without transferring ownership of the whole `RuntimeStepOutput` value.
pub struct RuntimeStepOutputSink<'a> {
    output: &'a mut RuntimeStepOutput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiagnostic {
    pub message: String,
}

/// Named value provided by adapters or earlier runtime operations.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputEvent {
    pub kind: String,
    pub payload: Option<String>,
}

/// UI event placeholder kept as Sans I/O data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiEvent {
    pub kind: String,
    pub payload: Option<String>,
}

/// Audio event placeholder kept as Sans I/O data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioEvent {
    pub kind: String,
    pub payload: Option<String>,
}

impl RuntimeStepInput {
    pub fn as_view(&self) -> RuntimeStepInputRef<'_> {
        RuntimeStepInputRef {
            tick: self.tick,
            dt: self.dt,
            bindings: self.bindings.as_slice(),
            input_events: self.input_events.as_slice(),
            task_events: self.task_events.as_slice(),
            ui_events: self.ui_events.as_slice(),
            audio_events: self.audio_events.as_slice(),
            source_events: self.source_events.as_slice(),
        }
    }
}

impl<'a> RuntimeStepInputRef<'a> {
    pub const fn tick(&self) -> TickId {
        self.tick
    }

    pub const fn dt(&self) -> LogicalDuration {
        self.dt
    }

    pub const fn bindings(&self) -> &'a [RuntimeBinding] {
        self.bindings
    }

    pub const fn input_events(&self) -> &'a [InputEvent] {
        self.input_events
    }

    pub const fn task_events(&self) -> &'a [TaskEvent] {
        self.task_events
    }

    pub const fn ui_events(&self) -> &'a [UiEvent] {
        self.ui_events
    }

    pub const fn audio_events(&self) -> &'a [AudioEvent] {
        self.audio_events
    }

    pub const fn source_events(&self) -> &'a [RuntimeSourceEvent] {
        self.source_events
    }
}

impl RuntimeStepOutput {
    pub fn writer(&mut self) -> RuntimeStepOutputSink<'_> {
        RuntimeStepOutputSink::new(self)
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.diagnostics.extend(other.diagnostics);
        self.flow_events.extend(other.flow_events);
        self.effects.line.extend(other.effects.line);
        self.effects
            .source_events
            .extend(other.effects.source_events);
        self.effects
            .stream_events
            .extend(other.effects.stream_events);
        self.requests.tasks.extend(other.requests.tasks);
        self.requests
            .cancel_scopes
            .extend(other.requests.cancel_scopes);
        self.requests
            .source_close
            .extend(other.requests.source_close);
    }
}

impl Default for RuntimeStepBudget {
    fn default() -> Self {
        Self { max_ops: 1 }
    }
}

impl<'a> RuntimeStepOutputSink<'a> {
    pub const fn new(output: &'a mut RuntimeStepOutput) -> Self {
        Self { output }
    }

    pub fn output(&self) -> &RuntimeStepOutput {
        self.output
    }

    pub fn output_mut(&mut self) -> &mut RuntimeStepOutput {
        self.output
    }

    pub fn push_diagnostic(&mut self, message: impl Into<String>) {
        self.output.diagnostics.push(RuntimeDiagnostic {
            message: message.into(),
        });
    }

    pub fn merge(&mut self, other: RuntimeStepOutput) {
        self.output.merge(other);
    }
}
