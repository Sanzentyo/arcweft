//! Product AWBC runtime-step parity adapter.
//!
//! This module is the sole adapter from canonical compact AWBC execution into
//! the shared `RuntimeStepResult` boundary. It is Sans I/O: every host action is
//! returned as typed data and no structured bytecode fallback is reachable.

mod audio;
mod control;
mod execution;
mod lifecycle;
mod mapping;
mod root;
mod runtime_id;
mod snapshot;
mod suspension;

use self::execution::{
    ProductVmHost, entry_argument_diagnostic, has_host_requests, has_visible_output,
    input_choice_selection, run_function, source_id_for, stream_id_for,
};
use self::mapping::{MappedEffect, content_request, source_diagnostic, task_spec};
use self::runtime_id::line_id_from_awbc_public_id;
pub use self::snapshot::{
    AwbcProductActiveChoiceSnapshot, AwbcProductActiveDialogueSnapshot,
    AwbcProductExecutorSnapshot, AwbcProductPendingHostCallSnapshot,
};
use crate::awbc::fiber::{
    FiberAwaitManyInFlight, FiberAwaitTarget, FiberBudget, FiberCursor, FiberState, FiberStatus,
    FiberSuspensionReason, FiberTerminalValue, FiberTrap,
};
use crate::awbc::schema::{
    AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcEffectPlanId, AwbcEntryId, AwbcFrameSlotRole,
    AwbcFunctionId, AwbcHostCallId, AwbcHostCallMode, AwbcLineTaskGroupId, AwbcLineTaskNode,
    AwbcLineTaskNodeId, AwbcLineTaskTrigger, AwbcProgram, AwbcResumePointId, AwbcSourceEventKind,
    AwbcSourcePlanId, AwbcStreamPlanId, AwbcTaskPlanId, AwbcTrapCode,
};
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::awbc::vm::{VmExit, VmObservation, VmStepOptions, step_with_host};
use crate::engine::{
    AwaitManyInFlight, AwaitManyState, AwaitState, ChoiceState, DialogueState, FlowExit, FlowFiber,
    FlowFiberStatus, HostCallState,
};
use crate::observation::RuntimeObservationState;
use crate::plan::{ChoiceRuntimeOption, FlowEvent, RuntimeHostCallTarget};
use crate::pure::{RuntimeCallBackend, VmRuntimePureCallBackend};
use crate::root::RootRuntime;
use crate::source::{
    RuntimeSourceEvent, SourceEventKind, SourceId, SourceRuntimeState, normalize_source_events,
};
use crate::step::{
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode,
    RuntimeHostCallRequest, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
    RuntimeStepOutput, RuntimeStepResult, RuntimeStepStats, RuntimeStepStopReason,
    input_event_trigger_name,
};
use crate::stream::{RuntimeStreamEvent, StreamRuntimeState};
use crate::task::{
    AwaitManyTarget, AwaitTarget, HostTaskRequestTemplate, NeedId, RuntimeNeedState, TaskEvent,
    TaskEventKind, TaskId, TaskKey, TaskSequence, normalize_runtime_need_states,
    normalize_task_events, resolved_runtime_need_state,
};
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeBinding, RuntimeEnv, RuntimePayload, RuntimeValue, runtime_sequence_from_literal_values,
    runtime_sequence_values, runtime_value_label,
};
use arcweft_interaction_model::audio::{AudioCommandEnvelope, AudioDispatchId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

/// Product AWBC executor construction failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwbcProductStepBuildError {
    #[error("product AWBC program failed verification: {message}")]
    InvalidProgram { message: String },
    #[error("failed to initialize product AWBC fiber state: {message}")]
    FiberState { message: String },
    #[error("failed to initialize product AWBC root state: {message}")]
    RootStartup { message: String },
    #[error("failed to restore product AWBC executor snapshot: {message}")]
    RestoreSnapshot { message: String },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(super) enum ProductStepError {
    #[error("{0}")]
    Input(String),
    #[error("{0}")]
    Type(String),
    #[error("{0}")]
    Host(String),
    #[error("{0}")]
    Internal(String),
}

impl ProductStepError {
    const fn category(&self) -> RuntimeDiagnosticCategory {
        match self {
            Self::Input(_) => RuntimeDiagnosticCategory::Input,
            Self::Type(_) => RuntimeDiagnosticCategory::Type,
            Self::Host(_) => RuntimeDiagnosticCategory::Host,
            Self::Internal(_) => RuntimeDiagnosticCategory::Internal,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ActiveDialogue {
    content: AwbcContentUnitId,
    group: AwbcLineTaskGroupId,
    started_nodes: BTreeSet<AwbcLineTaskNodeId>,
    elapsed_nanos: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct ActiveChoice {
    choice: AwbcChoiceId,
    public_id: Option<String>,
    options: Vec<ChoiceRuntimeOption>,
    option_indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingHostCall {
    call: AwbcHostCallId,
    id: RuntimeHostCallId,
}

/// Stateful canonical AWBC executor exposed through `RuntimeStepResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductStepExecutor {
    pub(super) program: AwbcProgram,
    fiber: FiberState,
    facade_fiber: FlowFiber,
    entry_bound: bool,
    active_dialogue: Option<ActiveDialogue>,
    active_choice: Option<ActiveChoice>,
    pending_host_call: Option<PendingHostCall>,
    started_tasks: BTreeSet<TaskId>,
    emitted_content: BTreeSet<AwbcContentUnitId>,
    stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    child_fibers: VecDeque<FiberState>,
    next_generation: u64,
    next_host_call_sequence: u64,
    next_audio_sequence: u64,
    compact_pure_stats: crate::step::RuntimePureCallStats,
    root: Option<RootRuntime>,
    root_flow_binding_name: Option<String>,
}

impl AwbcProductStepExecutor {
    /// Rebinds a verified code-compatible program without rerunning entry
    /// startup or replacing durable root/fiber state.
    pub fn replace_program_preserving_state(
        &mut self,
        program: AwbcProgram,
    ) -> Result<(), AwbcProductStepBuildError> {
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        let snapshot = self.snapshot();
        let mut candidate = self.clone();
        candidate.program = program;
        candidate.validate_snapshot(&snapshot)?;
        candidate.rebuild_facade_source_states_from_compact();
        candidate.rebuild_facade_stream_states_from_compact();
        candidate.sync_facade();
        *self = candidate;
        Ok(())
    }

    pub fn for_entry(
        program: AwbcProgram,
        entry: AwbcEntryId,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProductStepBuildError> {
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        let mut root_startup = root::prepare_startup(&program, entry)?;
        let mut fiber = if program.entries.is_empty() {
            FiberState {
                generation: 0,
                entry,
                cursor: FiberCursor {
                    function: AwbcFunctionId::default(),
                    block: AwbcBlockId::default(),
                    instruction_offset: 0,
                },
                frames: Vec::new(),
                status: FiberStatus::Returned,
                suspension: None,
                terminal: Some(FiberTerminalValue::Returned(None)),
                budget: FiberBudget {
                    remaining: budget_quantum,
                    quantum: budget_quantum,
                },
                line_cursor: 0,
                sources: Vec::new(),
                streams: Vec::new(),
            }
        } else {
            FiberState::for_entry(&program, entry, 0, budget_quantum.max(1)).map_err(|error| {
                AwbcProductStepBuildError::FiberState {
                    message: error.to_string(),
                }
            })?
        };
        root::bind_startup(&program, &mut fiber, root_startup.as_ref())?;
        let mut executor = Self::for_fiber(program, fiber);
        if let Some(startup) = root_startup.take() {
            executor.install_root_startup(startup);
        }
        Ok(executor)
    }

    pub fn for_function(
        program: AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProductStepBuildError> {
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        let fiber = FiberState::for_function(&program, entry, function, 0, budget_quantum.max(1))
            .map_err(|error| AwbcProductStepBuildError::FiberState {
            message: error.to_string(),
        })?;
        Ok(Self::for_fiber(program, fiber))
    }

    fn for_fiber(program: AwbcProgram, fiber: FiberState) -> Self {
        let mut facade_fiber = FlowFiber {
            line_cursor: 0,
            cursor: None,
            pending_ops: VecDeque::new(),
            control_stack: Vec::new(),
            root_cleanups: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
            status: if matches!(fiber.status, FiberStatus::Returned | FiberStatus::Cancelled) {
                FlowFiberStatus::Done(FlowExit::Done)
            } else {
                FlowFiberStatus::Running
            },
        };
        for (index, source) in program.source_plans.iter().enumerate() {
            let Some(index) = u32::try_from(index).ok() else {
                continue;
            };
            let id = source_id_for(&program, AwbcSourcePlanId(index));
            facade_fiber.source_states.insert(
                id.clone(),
                SourceRuntimeState::new(id, source.policy.runtime_policy()),
            );
        }
        for (index, _) in program.stream_plans.iter().enumerate() {
            let Some(index) = u32::try_from(index).ok() else {
                continue;
            };
            let id = stream_id_for(&program, AwbcStreamPlanId(index));
            facade_fiber
                .stream_states
                .insert(id.clone(), StreamRuntimeState::new(id));
        }
        Self {
            program,
            fiber,
            facade_fiber,
            entry_bound: false,
            active_dialogue: None,
            active_choice: None,
            pending_host_call: None,
            started_tasks: BTreeSet::new(),
            emitted_content: BTreeSet::new(),
            stream_sequences: BTreeMap::new(),
            child_fibers: VecDeque::new(),
            next_generation: 1,
            next_host_call_sequence: 0,
            next_audio_sequence: 0,
            compact_pure_stats: crate::step::RuntimePureCallStats::default(),
            root: None,
            root_flow_binding_name: None,
        }
    }

    pub const fn program(&self) -> &AwbcProgram {
        &self.program
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.facade_fiber
    }

    pub const fn compact_fiber(&self) -> &FiberState {
        &self.fiber
    }

    pub fn step(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        let mut backend = VmRuntimePureCallBackend::default();
        self.step_with_pure_backend(input, options, &mut backend)
    }

    pub fn step_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.step_with_root_bindings_and_pure_backend(input, &[], options, pure_backend)
    }

    #[allow(clippy::too_many_lines)]
    pub fn step_with_root_bindings_and_pure_backend(
        &mut self,
        mut input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        let pure_before = pure_backend.stats();
        let local_pure_before = self.compact_pure_stats;
        let mut output = RuntimeStepOutput::default();
        let mut executed_ops = 0_usize;
        let pending_ops_before = self.pending_ops_len();
        let root_events_in = input.root_events.len();
        let deferred_root_events = std::mem::take(&mut input.deferred_root_events);

        self.bind_facade_inputs_preserving_root_flow(root_bindings, &input.bindings);
        if !self.entry_bound && !self.fiber.frames.is_empty() {
            let mut entry_bindings = root_bindings.to_vec();
            for binding in &input.bindings {
                if let Some(existing) = entry_bindings
                    .iter_mut()
                    .find(|existing| existing.name == binding.name)
                {
                    *existing = binding.clone();
                } else {
                    entry_bindings.push(binding.clone());
                }
            }
            if let Err(error) = self.bind_root_arguments(&entry_bindings) {
                output.diagnostics.push(entry_argument_diagnostic(&error));
                self.sync_facade();
                return self.finish_result(
                    output,
                    RuntimeStepStopReason::Failed,
                    RuntimeStepStats {
                        pending_ops_before,
                        pending_ops_after: self.pending_ops_len(),
                        diagnostics: 1,
                        pure: pure_backend
                            .stats()
                            .saturating_delta(pure_before)
                            .saturating_add(
                                self.compact_pure_stats.saturating_delta(local_pure_before),
                            ),
                        ..RuntimeStepStats::default()
                    },
                );
            }
            self.entry_bound = true;
        }

        if !self.run_root_phase(
            std::mem::take(&mut input.root_events),
            &mut output,
            pure_backend,
        ) {
            self.sync_facade();
            let root_transitions = output.root_transitions.len();
            let root_commands = output.root_commands.len();
            let diagnostics = output.diagnostics.len();
            let stop_reason = self.stop_reason(options, executed_ops, &output);
            return self.finish_result(
                output,
                stop_reason,
                RuntimeStepStats {
                    pending_ops_before,
                    pending_ops_after: self.pending_ops_len(),
                    child_fibers: self.child_fibers.len(),
                    pure: pure_backend
                        .stats()
                        .saturating_delta(pure_before)
                        .saturating_add(
                            self.compact_pure_stats.saturating_delta(local_pure_before),
                        ),
                    root_events_in,
                    root_transitions,
                    root_commands,
                    diagnostics,
                    ..RuntimeStepStats::default()
                },
            );
        }
        output
            .requests
            .root_events_next_step
            .extend(deferred_root_events);

        let need_states = normalize_runtime_need_states(std::mem::take(&mut input.need_states));
        let task_events = normalize_task_events(std::mem::take(&mut input.task_events));
        let source_events = normalize_source_events(std::mem::take(&mut input.source_events));
        let need_states_in = need_states.len();
        let task_events_in = task_events.len();
        let source_events_in = source_events.len();
        output.diagnostics.extend(task_events.iter().map(|event| {
            RuntimeDiagnostic::new(format!(
                "task {} sequence {} delivered",
                event.task_id.0, event.sequence.0
            ))
        }));
        self.apply_source_events(source_events, &mut output);
        self.step_stream_plans(&mut output, pure_backend);

        if matches!(
            self.fiber
                .suspension
                .as_ref()
                .map(|suspension| &suspension.reason),
            Some(FiberSuspensionReason::BudgetYield)
        ) {
            if let Err(error) = self.fiber.resume_budget_yield(&self.program) {
                self.fail_with_error(ProductStepError::Internal(error.to_string()), &mut output);
            } else {
                self.fiber.replenish_budget();
            }
        } else if self.fiber.status == FiberStatus::Running {
            self.fiber.replenish_budget();
        }

        let max_ops = options.budget.max_ops;
        while executed_ops < max_ops && self.has_attemptable_work() {
            if self.fiber.status == FiberStatus::Suspended {
                let progressed = self.resume_main_suspension(
                    &input,
                    &need_states,
                    &task_events,
                    &mut output,
                    pure_backend,
                );
                executed_ops = executed_ops.saturating_add(usize::from(progressed));
                if !progressed || self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            }

            if self.fiber.status == FiberStatus::Running {
                let line_effects_before = output.effects.line.len();
                let step = self.step_main_vm(&need_states, &mut output, pure_backend);
                executed_ops = executed_ops.saturating_add(step);
                self.apply_control_effects(&mut output, line_effects_before);
            } else {
                let line_effects_before = output.effects.line.len();
                if !self.step_next_child(&mut output, pure_backend) {
                    break;
                }
                executed_ops = executed_ops.saturating_add(1);
                self.apply_control_effects(&mut output, line_effects_before);
            }

            if self.should_return_to_host(options.mode, &output, executed_ops) {
                break;
            }
        }

        for effect in &output.effects.line {
            self.facade_fiber.observations.record_effect(effect);
        }
        self.sync_facade();
        let stop_reason = self.stop_reason(options, executed_ops, &output);
        let stats = RuntimeStepStats {
            executed_ops,
            pending_ops_before,
            pending_ops_after: self.pending_ops_len(),
            child_fibers: self.child_fibers.len(),
            pure: pure_backend
                .stats()
                .saturating_delta(pure_before)
                .saturating_add(self.compact_pure_stats.saturating_delta(local_pure_before)),
            task_events_in,
            need_states_in,
            source_events_in,
            root_events_in,
            root_transitions: output.root_transitions.len(),
            root_commands: output.root_commands.len(),
            root_events_deferred: output.requests.root_events_next_step.len(),
            source_events_emitted: output.effects.source_events.len(),
            stream_events_emitted: output.effects.stream_events.len(),
            line_effects: output.effects.line.len(),
            audio_commands: output.requests.audio.len(),
            diagnostics: output.diagnostics.len(),
        };
        self.finish_result(output, stop_reason, stats)
    }

    fn finish_result(
        &self,
        output: RuntimeStepOutput,
        stop_reason: RuntimeStepStopReason,
        stats: RuntimeStepStats,
    ) -> RuntimeStepResult {
        RuntimeStepResult {
            output,
            fiber_status: self.effective_status(),
            stop_reason,
            stats,
        }
    }

    fn pending_ops_len(&self) -> usize {
        usize::from(self.fiber.status == FiberStatus::Running)
            .saturating_add(self.child_fibers.len())
    }

    fn has_attemptable_work(&self) -> bool {
        !matches!(
            self.fiber.status,
            FiberStatus::Returned | FiberStatus::Cancelled | FiberStatus::Trapped
        ) || self
            .child_fibers
            .iter()
            .any(|fiber| fiber.status == FiberStatus::Running)
    }

    fn step_main_vm(
        &mut self,
        need_states: &[RuntimeNeedState],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> usize {
        let mut host = ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut self.compact_pure_stats,
        };
        match step_with_host(
            &self.program,
            &mut self.fiber,
            VmStepOptions {
                max_instructions: 1,
            },
            &mut host,
        ) {
            Ok(vm_output) => {
                self.consume_observations(vm_output.observations, output);
                match vm_output.exit {
                    VmExit::Suspended(_) => {
                        self.sync_facade();
                        self.initialize_suspension(need_states, output, pure_backend);
                    }
                    VmExit::Returned(value) => Self::record_return(value.as_ref(), output),
                    VmExit::Running
                    | VmExit::Cancelled
                    | VmExit::Trapped(_)
                    | VmExit::BudgetYield(_) => {}
                }
                usize::try_from(vm_output.executed).unwrap_or(usize::MAX)
            }
            Err(error) => {
                self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                1
            }
        }
    }

    fn step_stream_plans(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let transforms = self
            .program
            .stream_plans
            .iter()
            .map(|stream| stream.transform)
            .collect::<Vec<_>>();
        for transform in transforms {
            self.step_stream_transform(transform, output, pure_backend);
        }
    }

    fn step_stream_transform(
        &mut self,
        transform: AwbcFunctionId,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let mut fiber =
            match FiberState::for_function(&self.program, self.fiber.entry, transform, 0, 64) {
                Ok(fiber) => fiber,
                Err(error) => {
                    self.record_error(ProductStepError::Internal(error.to_string()), output);
                    return;
                }
            };
        loop {
            let step = {
                let mut host = ProductVmHost {
                    backend: pure_backend,
                    fallback_stats: &mut self.compact_pure_stats,
                };
                step_with_host(
                    &self.program,
                    &mut fiber,
                    VmStepOptions {
                        max_instructions: 64,
                    },
                    &mut host,
                )
            };
            match step {
                Ok(vm_output) => {
                    self.consume_observations(vm_output.observations, output);
                    match vm_output.exit {
                        VmExit::Running => {}
                        VmExit::Returned(_) | VmExit::Cancelled => return,
                        VmExit::Trapped(trap) => {
                            self.record_trap(&trap, output);
                            return;
                        }
                        VmExit::Suspended(reason) => {
                            self.record_error(
                                ProductStepError::Internal(format!(
                                    "stream transform suspended at {reason:?}"
                                )),
                                output,
                            );
                            return;
                        }
                        VmExit::BudgetYield(_) => {
                            self.record_error(
                                ProductStepError::Internal(
                                    "stream transform exhausted compact budget".to_owned(),
                                ),
                                output,
                            );
                            return;
                        }
                    }
                }
                Err(error) => {
                    self.record_error(ProductStepError::Internal(error.to_string()), output);
                    return;
                }
            }
        }
    }

    fn record_return(value: Option<&RuntimeValue>, output: &mut RuntimeStepOutput) {
        match value {
            Some(value) => output.flow_events.push(FlowEvent::Return {
                value: runtime_value_label(value),
            }),
            None => output.flow_events.push(FlowEvent::Done),
        }
    }

    fn step_next_child(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(mut child) = self.child_fibers.pop_front() else {
            return false;
        };
        if child.status != FiberStatus::Running {
            self.child_fibers.push_back(child);
            return false;
        }
        child.replenish_budget();
        let mut host = ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut self.compact_pure_stats,
        };
        match step_with_host(
            &self.program,
            &mut child,
            VmStepOptions {
                max_instructions: 1,
            },
            &mut host,
        ) {
            Ok(vm_output) => {
                self.consume_observations(vm_output.observations, output);
                match child.status {
                    FiberStatus::Running | FiberStatus::Suspended => {
                        self.child_fibers.push_back(child);
                    }
                    FiberStatus::Returned | FiberStatus::Cancelled => {}
                    FiberStatus::Trapped => {
                        if let Some(FiberTerminalValue::Trapped(trap)) = child.terminal {
                            self.terminate_with_trap(trap, output);
                        }
                    }
                }
            }
            Err(error) => {
                self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
            }
        }
        true
    }

    fn initialize_suspension(
        &mut self,
        need_states: &[RuntimeNeedState],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let Some(suspension) = self.fiber.suspension.clone() else {
            return;
        };
        let declared_resume = suspension.declared_resume();
        match suspension.reason {
            FiberSuspensionReason::Dialogue {
                content,
                line_task_group,
            } => self.present_dialogue(content, line_task_group, output),
            FiberSuspensionReason::Choice { choice, .. } => {
                self.present_choice(choice, output, pure_backend);
            }
            FiberSuspensionReason::Await { target, binding } => match target {
                FiberAwaitTarget::Task(task) => self.ensure_await_started(&task, output),
                FiberAwaitTarget::Need(need) => {
                    if let Some(resume) = declared_resume {
                        self.resume_need(&need, binding, resume, need_states, output);
                    }
                }
            },
            FiberSuspensionReason::AwaitMany(_) => self.fill_await_many(output),
            FiberSuspensionReason::HostCall { call, args, .. } => {
                self.emit_host_call(call, &args, output);
            }
            FiberSuspensionReason::BudgetYield => {}
        }
    }

    fn resume_main_suspension(
        &mut self,
        input: &RuntimeStepInput,
        need_states: &[RuntimeNeedState],
        task_events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(suspension) = self.fiber.suspension.clone() else {
            return false;
        };
        if suspension.reason == FiberSuspensionReason::BudgetYield {
            return false;
        }
        let Some(resume) = suspension.declared_resume() else {
            self.fail_with_error(
                ProductStepError::Internal(
                    "non-budget suspension is missing a declared resume point".to_owned(),
                ),
                output,
            );
            return false;
        };
        match suspension.reason {
            FiberSuspensionReason::Dialogue {
                content,
                line_task_group,
            } => self.resume_dialogue(content, line_task_group, resume, input, output),
            FiberSuspensionReason::Choice {
                choice,
                destination,
            } => self.resume_choice(choice, destination, resume, input, output, pure_backend),
            FiberSuspensionReason::Await { target, binding } => match target {
                FiberAwaitTarget::Task(task) => {
                    self.resume_await(&task, binding, resume, task_events, output)
                }
                FiberAwaitTarget::Need(need) => {
                    self.resume_need(&need, binding, resume, need_states, output)
                }
            },
            FiberSuspensionReason::AwaitMany(state) => {
                self.resume_await_many(state, resume, task_events, output)
            }
            FiberSuspensionReason::HostCall {
                call, destination, ..
            } => self.resume_host_call(call, destination, resume, &input.host_call_results, output),
            FiberSuspensionReason::BudgetYield => false,
        }
    }

    fn present_dialogue(
        &mut self,
        content: AwbcContentUnitId,
        group: AwbcLineTaskGroupId,
        output: &mut RuntimeStepOutput,
    ) {
        if self
            .active_dialogue
            .as_ref()
            .is_some_and(|active| active.content == content && active.group == group)
        {
            return;
        }
        let line = self.content_public_id(content);
        let line_id = match line_id_from_awbc_public_id(&line) {
            Ok(line_id) => line_id,
            Err(error) => {
                self.record_error(error, output);
                return;
            }
        };
        output.flow_events.push(FlowEvent::DialogueLine {
            line: line_id,
            bindings: self.facade_fiber.env.bindings_snapshot(),
        });
        self.emitted_content.insert(content);
        let mut active = ActiveDialogue {
            content,
            group,
            started_nodes: BTreeSet::new(),
            elapsed_nanos: 0,
        };
        self.run_line_task_nodes(&mut active, None, output);
        self.active_dialogue = Some(active);
        self.fiber.line_cursor = self.fiber.line_cursor.saturating_add(1);
    }

    fn resume_dialogue(
        &mut self,
        content: AwbcContentUnitId,
        group: AwbcLineTaskGroupId,
        resume: AwbcResumePointId,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if self.active_dialogue.is_none() {
            self.present_dialogue(content, group, output);
        }
        let mut active = self.active_dialogue.take().unwrap_or(ActiveDialogue {
            content,
            group,
            started_nodes: BTreeSet::new(),
            elapsed_nanos: 0,
        });
        active.elapsed_nanos = active.elapsed_nanos.saturating_add(input.dt.as_nanos());
        self.run_line_task_nodes(&mut active, Some(input), output);
        if let Some(trigger) = self.dialogue_cancel_trigger(group, input) {
            output.flow_events.push(FlowEvent::LineCancelled {
                trigger: trigger.clone(),
            });
            self.spawn_cancel_handler(group, &trigger, output);
            self.cleanup_dialogue(group, output);
            self.active_dialogue = None;
            return self.resume_at(resume, output);
        }
        let line = self.content_public_id(content);
        if input.advances_dialogue_label(&line) {
            self.cleanup_dialogue(group, output);
            self.active_dialogue = None;
            return self.resume_at(resume, output);
        }
        self.active_dialogue = Some(active);
        false
    }

    fn run_line_task_nodes(
        &mut self,
        active: &mut ActiveDialogue,
        input: Option<&RuntimeStepInput>,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(group) = self.program.line_task_groups.get(active.group.index()) else {
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Internal,
                format!("missing AWBC line task group {}", active.group.0),
            ));
            return;
        };
        self.run_line_task_node(group.root, active, input, output);
    }

    fn run_line_task_node(
        &mut self,
        node: AwbcLineTaskNodeId,
        active: &mut ActiveDialogue,
        input: Option<&RuntimeStepInput>,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(record) = self.program.line_task_nodes.get(node.index()).cloned() else {
            return;
        };
        match record {
            AwbcLineTaskNode::Sequence(children)
            | AwbcLineTaskNode::Start(children)
            | AwbcLineTaskNode::Parallel { children, .. } => {
                for child in children {
                    self.run_line_task_node(child, active, input, output);
                }
            }
            AwbcLineTaskNode::Effect(effect) => {
                if active.started_nodes.insert(node) {
                    self.emit_effect(effect, &[], output);
                }
            }
            AwbcLineTaskNode::Child {
                task,
                trigger,
                scope,
                ..
            } => {
                let triggered = match trigger {
                    AwbcLineTaskTrigger::Immediate => true,
                    AwbcLineTaskTrigger::Mark(mark) => input.is_some_and(|input| {
                        let mark = self
                            .program
                            .strings
                            .get(mark.index())
                            .map(String::as_str)
                            .unwrap_or_default();
                        input.input_events.iter().any(|event| {
                            input_event_trigger_name(event).is_some_and(|value| {
                                value == mark || value == format!("mark:{mark}")
                            })
                        })
                    }),
                    AwbcLineTaskTrigger::DelayNanos(nanos) => active.elapsed_nanos >= nanos,
                };
                if triggered && active.started_nodes.insert(node) {
                    let task_id = TaskId(self.task_public_id(task));
                    match task_spec(&self.program, task, &task_id, Vec::new()) {
                        Ok((need, spec)) => {
                            output.flow_events.push(FlowEvent::AwaitStarted {
                                need,
                                task: task_id.clone(),
                            });
                            output.requests.tasks.push(spec);
                            self.started_tasks.insert(task_id);
                        }
                        Err(error) => self.record_error(error, output),
                    }
                    self.run_line_task_node(scope, active, input, output);
                }
            }
        }
    }

    fn dialogue_cancel_trigger(
        &self,
        group: AwbcLineTaskGroupId,
        input: &RuntimeStepInput,
    ) -> Option<String> {
        let group = self.program.line_task_groups.get(group.index())?;
        group.cancel_handlers.iter().find_map(|handler| {
            let trigger = self.program.strings.get(handler.trigger.index())?;
            input
                .input_events
                .iter()
                .any(|event| input_event_trigger_name(event) == Some(trigger.as_str()))
                .then(|| trigger.clone())
        })
    }

    fn spawn_cancel_handler(
        &mut self,
        group: AwbcLineTaskGroupId,
        trigger: &str,
        output: &mut RuntimeStepOutput,
    ) {
        let function = self
            .program
            .line_task_groups
            .get(group.index())
            .and_then(|group| {
                group.cancel_handlers.iter().find_map(|handler| {
                    self.program
                        .strings
                        .get(handler.trigger.index())
                        .filter(|candidate| candidate.as_str() == trigger)
                        .map(|_| handler.function)
                })
            });
        if let Some(function) = function {
            self.spawn_child(function, &[], output);
        }
    }

    fn cleanup_dialogue(&mut self, group: AwbcLineTaskGroupId, output: &mut RuntimeStepOutput) {
        let Some(group) = self.program.line_task_groups.get(group.index()) else {
            return;
        };
        if matches!(
            group.cleanup.child_tasks,
            crate::awbc::schema::AwbcChildCleanup::CancelAndJoin
        ) {
            for task in &self.program.task_plans {
                if let Some(scope) = self.program.strings.get(task.cancel_scope.index()) {
                    output
                        .requests
                        .cancel_scopes
                        .push(crate::task::CancelScopeId(scope.clone()));
                }
            }
        }
    }

    fn present_choice(
        &mut self,
        choice: AwbcChoiceId,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if self
            .active_choice
            .as_ref()
            .is_some_and(|active| active.choice == choice)
        {
            return;
        }
        let Some(record) = self.program.choices.get(choice.index()).cloned() else {
            self.fail_with_error(
                ProductStepError::Internal(format!("missing AWBC choice {}", choice.0)),
                output,
            );
            return;
        };
        let start = record.options.start as usize;
        let end = start.saturating_add(record.options.len as usize);
        let candidates =
            self.program.choice_options[start..end.min(self.program.choice_options.len())].to_vec();
        let mut options = Vec::new();
        let mut option_indices = Vec::new();
        for (relative_index, option) in candidates.into_iter().enumerate() {
            if let Some(condition) = option.condition {
                match run_function(
                    &self.program,
                    condition,
                    &[],
                    pure_backend,
                    &mut self.compact_pure_stats,
                ) {
                    Ok(RuntimeValue::Bool(true)) => {}
                    Ok(RuntimeValue::Bool(false)) => continue,
                    Ok(value) => {
                        self.record_error(
                            ProductStepError::Type(format!(
                                "choice condition returned {}, expected bool",
                                runtime_value_label(&value)
                            )),
                            output,
                        );
                        continue;
                    }
                    Err(error) => {
                        self.record_error(ProductStepError::Internal(error.to_string()), output);
                        continue;
                    }
                }
            }
            options.push(self.choice_runtime_option(&option));
            option_indices.push(start + relative_index);
        }
        let public_id = record
            .public_id
            .and_then(|id| self.program.strings.get(id.index()).cloned());
        output.flow_events.push(FlowEvent::ChoicePresented {
            id: public_id.clone(),
            options: options.clone(),
        });
        self.active_choice = Some(ActiveChoice {
            choice,
            public_id,
            options,
            option_indices,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn resume_choice(
        &mut self,
        choice: AwbcChoiceId,
        destination: crate::awbc::schema::AwbcRegisterId,
        resume: AwbcResumePointId,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        if self.active_choice.is_none() {
            self.present_choice(choice, output, pure_backend);
        }
        let Some((requested_choice, selection)) = input_choice_selection(input) else {
            return false;
        };
        let Some(active) = self.active_choice.clone() else {
            return false;
        };
        if requested_choice.is_some() && requested_choice != active.public_id.as_deref() {
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Input,
                format!(
                    "stale choice selection for `{}` while waiting on `{}`",
                    requested_choice.unwrap_or_default(),
                    active.public_id.as_deref().unwrap_or("-")
                ),
            ));
            return false;
        }
        let Some(position) = active.options.iter().position(|option| {
            option.id.as_deref() == Some(selection) || option.label == selection
        }) else {
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Input,
                format!(
                    "invalid option `{selection}` for choice `{}`",
                    active.public_id.as_deref().unwrap_or("-")
                ),
            ));
            return false;
        };
        let selected = active.options[position]
            .id
            .clone()
            .unwrap_or_else(|| active.options[position].label.clone());
        let option = self.program.choice_options[active.option_indices[position]].clone();
        if let Ok(frame) = self.fiber.active_frame_mut()
            && let Err(error) =
                frame.set_register(destination, RuntimeValue::String(selected.clone()))
        {
            self.record_error(ProductStepError::Internal(error.to_string()), output);
            return false;
        }
        output.flow_events.push(FlowEvent::ChoiceSelected {
            id: active.public_id,
            option: selected,
        });
        for effect in option.effects {
            self.emit_effect(effect, &[], output);
        }
        if let Some(effect) = option.out_effect {
            self.emit_effect(effect, &[], output);
        }
        self.active_choice = None;
        if !self.resume_at(resume, output) {
            return false;
        }
        if let Some(target) = option.target {
            if let Err(error) = self
                .fiber
                .replace_active_function(&self.program, target, &[])
            {
                self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                return false;
            }
            let target = match self.flow_identity_for_function(target) {
                Ok(target) => target,
                Err(error) => {
                    self.fail_with_error(error, output);
                    return false;
                }
            };
            output.flow_events.push(FlowEvent::Goto { target });
        }
        true
    }
}

#[cfg(test)]
mod tests;
