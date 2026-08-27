use crate::effect::LineEffectRequest;
use crate::plan::FlowEvent;
use crate::root::{RootEventInput, RootTransitionOutcome, RuntimeCommandEnvelope};
use crate::runtime_id::{DialogueActivationId, RuntimeDialogueEffectSiteId, RuntimeDialogueMarkId};
use crate::stream::RuntimeStreamEvent;
use crate::task::{CancelScopeId, NamedHostArg, RuntimeNeedState, TaskEvent, TaskSpec};
use crate::time::{LogicalDuration, TickId};
use crate::value::RuntimePayload;
use arcweft_interaction_model::{
    audio::{AudioCommandEnvelope, AudioEvent},
    input::{InputEventKind, RoutedInputEvent},
    payload::InteractionPayload,
};
pub use arcweft_manifest_model::HostCallContractDigest;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeStepInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub input_events: Vec<RoutedInputEvent>,
    pub dialogue_content_events: Vec<RuntimeDialogueContentEvent>,
    pub dialogue_advances: Vec<DialogueActivationId>,
    pub need_states: Vec<RuntimeNeedState>,
    pub task_events: Vec<TaskEvent>,
    pub audio_events: Vec<AudioEvent>,
    pub host_call_results: Vec<RuntimeHostCallResult>,
    pub line_outcomes: Vec<crate::presentation::RuntimeLineHostOutcome>,
    pub root_events: Vec<RootEventInput>,
    /// Typed events emitted by a later-phase owner for root ingress on the
    /// following step. They never participate in the current root phase.
    pub deferred_root_events: Vec<RootEventInput>,
}

/// Borrowed adapter-facing view of runtime step inputs.
///
/// Adapters should prefer this view when handing input data into lower runtime
/// layers. The view keeps ownership at the adapter step boundary and makes it
/// clear that runtime code must not retain borrowed event slices past the step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeStepInputRef<'a> {
    tick: TickId,
    dt: LogicalDuration,
    input_events: &'a [RoutedInputEvent],
    dialogue_content_events: &'a [RuntimeDialogueContentEvent],
    dialogue_advances: &'a [DialogueActivationId],
    need_states: &'a [RuntimeNeedState],
    task_events: &'a [TaskEvent],
    audio_events: &'a [AudioEvent],
    host_call_results: &'a [RuntimeHostCallResult],
    line_outcomes: &'a [crate::presentation::RuntimeLineHostOutcome],
    root_events: &'a [RootEventInput],
}

/// One source-ordered reveal event emitted by the presentation owner for an
/// exact dialogue activation. Marks and inline effect sites share this carrier
/// so neither can be reconstructed from a label or delivered to a later
/// activation of the same authored line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueContentEvent {
    activation: DialogueActivationId,
    kind: RuntimeDialogueContentEventKind,
}

impl RuntimeDialogueContentEvent {
    #[must_use]
    pub const fn new(
        activation: DialogueActivationId,
        kind: RuntimeDialogueContentEventKind,
    ) -> Self {
        Self { activation, kind }
    }

    #[must_use]
    pub const fn activation(&self) -> &DialogueActivationId {
        &self.activation
    }

    #[must_use]
    pub const fn kind(&self) -> RuntimeDialogueContentEventKind {
        self.kind
    }
}

/// Content-plan coordinate revealed within one dialogue activation.
#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
pub enum RuntimeDialogueContentEventKind {
    Mark(RuntimeDialogueMarkId),
    Effect(RuntimeDialogueEffectSiteId),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeStepOutput {
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub flow_events: Vec<FlowEvent>,
    pub effects: RuntimeEffectBatch,
    pub requests: HostRequestBatch,
    pub root_transitions: Vec<RootTransitionOutcome>,
    pub root_commands: Vec<RuntimeCommandEnvelope>,
}

/// Runtime-produced events and effect requests, kept as pure data for hosts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeEffectBatch {
    pub line: Vec<LineEffectRequest>,
    pub stream_events: Vec<RuntimeStreamEvent>,
}

/// Host-facing requests emitted by one deterministic runtime step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostRequestBatch {
    pub tasks: Vec<TaskSpec>,
    pub audio: Vec<AudioCommandEnvelope>,
    pub cancel_scopes: Vec<CancelScopeId>,
    pub ensure_content: Vec<RuntimeContentRequest>,
    pub host_calls: Vec<RuntimeHostCallRequest>,
    pub line_commands: Vec<crate::presentation::RuntimeLineHostCommand>,
    /// Events accepted from later-phase owners for deterministic next-step
    /// root ingress. The session driver owns the queue after this step.
    pub root_events_next_step: Vec<RootEventInput>,
}

/// Stable identifier for one host call request/result exchange.
#[derive(
    Clone,
    Debug,
    Default,
    serde::Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Serialize,
)]
pub struct RuntimeHostCallId(pub String);

/// Whether a host call may complete in the emitting step or requires a later
/// host result before the fiber can resume.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub enum RuntimeHostCallMode {
    #[default]
    Immediate,
    Suspend,
}

/// Typed host request emitted by compact or structured execution.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeHostCallRequest {
    pub id: RuntimeHostCallId,
    pub public_id: String,
    pub capability: String,
    pub operation: String,
    pub contract: Option<HostCallContractDigest>,
    pub args: Vec<RuntimePayload>,
    pub named_args: Vec<NamedHostArg<RuntimePayload>>,
    pub result: crate::pattern::RuntimeCheckedType,
    pub mode: RuntimeHostCallMode,
    pub deterministic: bool,
}

/// Host-supplied outcome for a previously emitted host call request.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeHostCallResult {
    pub id: RuntimeHostCallId,
    pub outcome: Result<RuntimePayload, RuntimeHostCallError>,
}

/// Typed host-call failure preserved at the deterministic step boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeHostCallError {
    pub kind: RuntimeHostCallErrorKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHostCallErrorKind {
    UnsupportedCapability,
    Rejected,
    Failed,
}

/// Content/resource residency request emitted by a Sans-I/O runtime step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeContentRequest {
    pub content: String,
    pub resources: Vec<RuntimeContentResourceRequest>,
}

/// One resource referenced by a content residency request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeContentResourceRequest {
    pub public_id: String,
    pub kind: String,
    pub digest: [u8; 32],
    pub decoded_len: u64,
    pub residency: RuntimeContentResidency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeContentResidency {
    Startup,
    OnDemand,
    Streaming,
}

/// Result envelope returned by the runtime step boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeStepResult {
    pub output: RuntimeStepOutput,
    pub fiber_status: crate::engine::FlowFiberStatus,
    pub stop_reason: RuntimeStepStopReason,
    pub stats: RuntimeStepStats,
}

/// Deterministic counters for one runtime step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeStepStats {
    pub executed_ops: usize,
    pub pending_ops_before: usize,
    pub pending_ops_after: usize,
    pub child_fibers: usize,
    pub pure: RuntimePureCallStats,
    pub task_events_in: usize,
    pub need_states_in: usize,
    pub root_events_in: usize,
    pub root_transitions: usize,
    pub root_commands: usize,
    pub root_events_deferred: usize,
    pub stream_events_emitted: usize,
    pub line_effects: usize,
    pub audio_commands: usize,
    pub diagnostics: usize,
}

/// Deterministic counters for runtime pure helper acceleration.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize, PartialEq, serde::Serialize)]
pub struct RuntimePureCallStats {
    /// Accepted top-level compact-AWBC pure-program invocations.
    ///
    /// This is non-semantic runtime telemetry. It is not persisted into
    /// replay, cache, or program identity state.
    pub awbc_pure_program_calls: usize,
    pub pure_calls: usize,
    pub math_calls: usize,
    pub math_accelerated_calls: usize,
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
    pub parallel_policy_checks: usize,
    pub parallel_work_units: usize,
    pub parallel_batches: usize,
    pub parallel_skipped_backend: usize,
    pub parallel_skipped_small: usize,
    pub thread_pool_jobs: usize,
    #[serde(with = "u128_string")]
    pub thread_pool_build_elapsed_ns: u128,
    pub fallbacks: usize,
}

mod u128_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse::<u128>().map_err(serde::de::Error::custom)
    }
}

impl RuntimePureCallStats {
    /// Adds two independently-accounted runtime call deltas without overflow.
    #[must_use]
    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            awbc_pure_program_calls: self
                .awbc_pure_program_calls
                .saturating_add(other.awbc_pure_program_calls),
            pure_calls: self.pure_calls.saturating_add(other.pure_calls),
            math_calls: self.math_calls.saturating_add(other.math_calls),
            math_accelerated_calls: self
                .math_accelerated_calls
                .saturating_add(other.math_accelerated_calls),
            batch_calls: self.batch_calls.saturating_add(other.batch_calls),
            batch_items: self.batch_items.saturating_add(other.batch_items),
            flat_batch_calls: self.flat_batch_calls.saturating_add(other.flat_batch_calls),
            flat_batch_items: self.flat_batch_items.saturating_add(other.flat_batch_items),
            flat_batch_bytes_borrowed: self
                .flat_batch_bytes_borrowed
                .saturating_add(other.flat_batch_bytes_borrowed),
            flatten_materializations: self
                .flatten_materializations
                .saturating_add(other.flatten_materializations),
            flatten_bytes_copied: self
                .flatten_bytes_copied
                .saturating_add(other.flatten_bytes_copied),
            jit_calls: self.jit_calls.saturating_add(other.jit_calls),
            aot_calls: self.aot_calls.saturating_add(other.aot_calls),
            vm_calls: self.vm_calls.saturating_add(other.vm_calls),
            arg_stack_packs: self.arg_stack_packs.saturating_add(other.arg_stack_packs),
            arg_vec_allocations: self
                .arg_vec_allocations
                .saturating_add(other.arg_vec_allocations),
            arg_bytes_copied: self.arg_bytes_copied.saturating_add(other.arg_bytes_copied),
            arg_bytes_borrowed: self
                .arg_bytes_borrowed
                .saturating_add(other.arg_bytes_borrowed),
            result_bytes_copied: self
                .result_bytes_copied
                .saturating_add(other.result_bytes_copied),
            parallel_policy_checks: self
                .parallel_policy_checks
                .saturating_add(other.parallel_policy_checks),
            parallel_work_units: self
                .parallel_work_units
                .saturating_add(other.parallel_work_units),
            parallel_batches: self.parallel_batches.saturating_add(other.parallel_batches),
            parallel_skipped_backend: self
                .parallel_skipped_backend
                .saturating_add(other.parallel_skipped_backend),
            parallel_skipped_small: self
                .parallel_skipped_small
                .saturating_add(other.parallel_skipped_small),
            thread_pool_jobs: self.thread_pool_jobs.saturating_add(other.thread_pool_jobs),
            thread_pool_build_elapsed_ns: self
                .thread_pool_build_elapsed_ns
                .saturating_add(other.thread_pool_build_elapsed_ns),
            fallbacks: self.fallbacks.saturating_add(other.fallbacks),
        }
    }

    #[must_use]
    pub fn saturating_delta(self, before: Self) -> Self {
        Self {
            awbc_pure_program_calls: self
                .awbc_pure_program_calls
                .saturating_sub(before.awbc_pure_program_calls),
            pure_calls: self.pure_calls.saturating_sub(before.pure_calls),
            math_calls: self.math_calls.saturating_sub(before.math_calls),
            math_accelerated_calls: self
                .math_accelerated_calls
                .saturating_sub(before.math_accelerated_calls),
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
            parallel_policy_checks: self
                .parallel_policy_checks
                .saturating_sub(before.parallel_policy_checks),
            parallel_work_units: self
                .parallel_work_units
                .saturating_sub(before.parallel_work_units),
            parallel_batches: self
                .parallel_batches
                .saturating_sub(before.parallel_batches),
            parallel_skipped_backend: self
                .parallel_skipped_backend
                .saturating_sub(before.parallel_skipped_backend),
            parallel_skipped_small: self
                .parallel_skipped_small
                .saturating_sub(before.parallel_skipped_small),
            thread_pool_jobs: self
                .thread_pool_jobs
                .saturating_sub(before.thread_pool_jobs),
            thread_pool_build_elapsed_ns: self
                .thread_pool_build_elapsed_ns
                .saturating_sub(before.thread_pool_build_elapsed_ns),
            fallbacks: self.fallbacks.saturating_sub(before.fallbacks),
        }
    }
}

/// Runtime stepping policy selected by hosts and CLI tooling.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeStepOptions {
    pub mode: RuntimeStepMode,
    pub budget: RuntimeStepBudget,
}

/// Deterministic work budget for one `Engine::step` call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeStepBudget {
    pub max_ops: usize,
}

/// Runtime drain strategy. Adapter loops remain outside `arcweft-core`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RuntimeStepMode {
    #[default]
    OneOp,
    Drain,
    Game,
    Server,
}

/// Reason the runtime stopped returning control to the host.
#[derive(Clone, Copy, Debug, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeDiagnostic {
    pub message: String,
    pub category: RuntimeDiagnosticCategory,
    pub source: Option<RuntimeDiagnosticSource>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeDiagnosticCategory {
    #[default]
    Runtime,
    Input,
    Type,
    Pattern,
    Host,
    Capability,
    Budget,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiagnosticSource {
    pub label: String,
    pub start: u32,
    pub end: u32,
    pub anchor: Option<String>,
}

impl RuntimeStepInput {
    pub fn as_view(&self) -> RuntimeStepInputRef<'_> {
        RuntimeStepInputRef {
            tick: self.tick,
            dt: self.dt,
            input_events: self.input_events.as_slice(),
            dialogue_content_events: self.dialogue_content_events.as_slice(),
            dialogue_advances: self.dialogue_advances.as_slice(),
            need_states: self.need_states.as_slice(),
            task_events: self.task_events.as_slice(),
            audio_events: self.audio_events.as_slice(),
            host_call_results: self.host_call_results.as_slice(),
            line_outcomes: self.line_outcomes.as_slice(),
            root_events: self.root_events.as_slice(),
        }
    }
}

pub(crate) fn input_event_trigger_name(event: &RoutedInputEvent) -> Option<&str> {
    match &event.event {
        InputEventKind::Custom { name } => Some(name.as_str()),
        InputEventKind::Text { .. } => Some("text"),
        InputEventKind::FocusGained => Some("focus_gained"),
        InputEventKind::FocusLost => Some("focus_lost"),
        InputEventKind::PointerMove { .. }
        | InputEventKind::PointerDown { .. }
        | InputEventKind::PointerUp { .. }
        | InputEventKind::Scroll { .. }
        | InputEventKind::KeyDown { .. }
        | InputEventKind::KeyUp { .. } => None,
    }
}

pub(crate) fn input_event_text_payload(event: &RoutedInputEvent) -> Option<&str> {
    match event.payload.as_ref()? {
        InteractionPayload::Text(value) => Some(value.as_str()),
        InteractionPayload::Entity(value) => Some(value.as_str()),
        InteractionPayload::Null
        | InteractionPayload::Bool(_)
        | InteractionPayload::I64(_)
        | InteractionPayload::U64(_)
        | InteractionPayload::F64(_)
        | InteractionPayload::List(_)
        | InteractionPayload::Map(_) => None,
    }
}

impl<'a> RuntimeStepInputRef<'a> {
    pub const fn tick(&self) -> TickId {
        self.tick
    }

    pub const fn dt(&self) -> LogicalDuration {
        self.dt
    }

    pub const fn input_events(&self) -> &'a [RoutedInputEvent] {
        self.input_events
    }

    pub const fn dialogue_content_events(&self) -> &'a [RuntimeDialogueContentEvent] {
        self.dialogue_content_events
    }

    pub const fn dialogue_advances(&self) -> &'a [DialogueActivationId] {
        self.dialogue_advances
    }

    pub const fn need_states(&self) -> &'a [RuntimeNeedState] {
        self.need_states
    }

    pub const fn task_events(&self) -> &'a [TaskEvent] {
        self.task_events
    }

    pub const fn audio_events(&self) -> &'a [AudioEvent] {
        self.audio_events
    }

    pub const fn host_call_results(&self) -> &'a [RuntimeHostCallResult] {
        self.host_call_results
    }

    pub const fn line_outcomes(&self) -> &'a [crate::presentation::RuntimeLineHostOutcome] {
        self.line_outcomes
    }

    pub const fn root_events(&self) -> &'a [RootEventInput] {
        self.root_events
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
            .stream_events
            .extend(other.effects.stream_events);
        self.requests.tasks.extend(other.requests.tasks);
        self.requests.audio.extend(other.requests.audio);
        self.requests
            .cancel_scopes
            .extend(other.requests.cancel_scopes);
        self.requests
            .ensure_content
            .extend(other.requests.ensure_content);
        self.requests.host_calls.extend(other.requests.host_calls);
        self.requests
            .line_commands
            .extend(other.requests.line_commands);
        self.root_transitions.extend(other.root_transitions);
        self.root_commands.extend(other.root_commands);
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
        self.output
            .diagnostics
            .push(RuntimeDiagnostic::new(message));
    }

    pub fn merge(&mut self, other: RuntimeStepOutput) {
        self.output.merge(other);
    }
}

impl RuntimeDiagnostic {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            category: RuntimeDiagnosticCategory::Runtime,
            source: None,
        }
    }

    #[must_use]
    pub fn categorized(category: RuntimeDiagnosticCategory, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            category,
            source: None,
        }
    }

    #[must_use]
    pub fn with_source(mut self, source: RuntimeDiagnosticSource) -> Self {
        self.source = Some(source);
        self
    }
}
