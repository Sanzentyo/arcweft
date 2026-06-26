//! Product AWBC runtime-step parity adapter.
//!
//! This module is the sole adapter from canonical compact AWBC execution into
//! the shared `RuntimeStepResult` boundary. It is Sans I/O: every host action is
//! returned as typed data and no structured bytecode fallback is reachable.

mod mapping;

use self::mapping::{MappedEffect, content_request, source_diagnostic, task_spec};
use crate::awbc::fiber::{
    FiberAwaitManyInFlight, FiberBudget, FiberCursor, FiberState, FiberStateError, FiberStatus,
    FiberSuspensionReason, FiberTerminalValue, FiberTrap,
};
use crate::awbc::schema::{
    AwbcBackpressurePolicy, AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcEffectPlanId,
    AwbcEntryId, AwbcFunctionId, AwbcHostCallId, AwbcHostCallMode, AwbcInstruction,
    AwbcLineTaskGroupId, AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger,
    AwbcOverflowPolicy, AwbcPrivacyPolicy, AwbcProgram, AwbcReplayPolicy, AwbcResumePointId,
    AwbcSourceEventKind, AwbcSourcePlanId, AwbcStreamPlanId, AwbcTaskPlanId, AwbcTerminator,
    AwbcTrapCode,
};
use crate::awbc::vm::{VmError, VmExit, VmHost, VmObservation, VmStepOptions, step_with_host};
use crate::engine::{
    AwaitManyInFlight, AwaitManyState, AwaitState, ChoiceState, DialogueState, FlowExit, FlowFiber,
    FlowFiberStatus,
};
use crate::observation::RuntimeObservationState;
use crate::plan::{ChoiceRuntimeOption, FlowEvent, FlowRuntimeId, RuntimeLineId};
use crate::pure::{RuntimeCallBackend, RuntimeCompactPureHelper, VmRuntimePureCallBackend};
use crate::source::{
    BackpressurePolicy, OverflowPolicy, PrivacyPolicy, ReplayPolicy, RuntimeSourceEvent,
    SourceEventKind, SourceId, SourcePolicy, SourceRuntimeState, normalize_source_events,
};
use crate::step::{
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode,
    RuntimeHostCallRequest, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
    RuntimeStepOutput, RuntimeStepResult, RuntimeStepStats, RuntimeStepStopReason,
    input_event_text_payload, input_event_trigger_name,
};
use crate::stream::{RuntimeStreamEvent, StreamRuntimeId, StreamRuntimeState};
use crate::task::{
    AwaitManyTarget, AwaitTarget, HostTaskRequestTemplate, NeedId, TaskEvent, TaskEventKind,
    TaskId, TaskKey, TaskSequence, normalize_task_events,
};
use crate::value::{
    RuntimeBinding, RuntimeCallTarget, RuntimeEnv, RuntimePayload, RuntimeValue,
    runtime_sequence_from_literal_values, runtime_value_label,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

/// Historical family names retained for source compatibility and audits.
///
/// Every supported canonical AWBC family is now implemented at this boundary,
/// so ordinary product programs report an empty blocker inventory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AwbcProductStepParityBlocker {
    EntryArguments,
    PureHelperCalls,
    IntrinsicCalls,
    TrapSourceReporting,
    ContentEnsures,
    Effects,
    TaskStarts,
    SpawnedFibers,
    Streams,
    Sources,
    Dialogue,
    Choice,
    Await,
    AwaitMany,
    HostCall,
    BudgetYield,
}

/// Product AWBC executor construction failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwbcProductStepBuildError {
    #[error("product AWBC runtime-step parity is incomplete: {blockers:?}")]
    UnsupportedParity {
        blockers: Vec<AwbcProductStepParityBlocker>,
    },
    #[error("failed to initialize product AWBC fiber state: {message}")]
    FiberState { message: String },
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

impl AwbcInstruction {
    /// All canonical instruction families are projected at the product step boundary.
    #[must_use]
    pub const fn product_step_parity_blocker(&self) -> Option<AwbcProductStepParityBlocker> {
        let _ = self;
        None
    }
}

impl AwbcTerminator {
    /// All canonical terminator families are projected at the product step boundary.
    #[must_use]
    pub const fn product_step_parity_blocker(&self) -> Option<AwbcProductStepParityBlocker> {
        let _ = self;
        None
    }
}

impl AwbcProgram {
    /// Returns blockers for the supported product language surface.
    #[must_use]
    pub fn product_step_parity_blockers(&self) -> Vec<AwbcProductStepParityBlocker> {
        let _ = self;
        Vec::new()
    }

    pub fn ensure_product_step_parity(&self) -> Result<(), AwbcProductStepBuildError> {
        let blockers = self.product_step_parity_blockers();
        if blockers.is_empty() {
            Ok(())
        } else {
            Err(AwbcProductStepBuildError::UnsupportedParity { blockers })
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
    source_sequences: BTreeMap<SourceId, TaskSequence>,
    stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    child_fibers: VecDeque<FiberState>,
    next_generation: u64,
    next_host_call_sequence: u64,
    compact_pure_stats: crate::step::RuntimePureCallStats,
}

impl AwbcProductStepExecutor {
    pub fn for_entry(
        program: AwbcProgram,
        entry: AwbcEntryId,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProductStepBuildError> {
        program.ensure_product_step_parity()?;
        let fiber = if program.entries.is_empty() {
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
        let mut facade_fiber = FlowFiber {
            line_cursor: 0,
            cursor: None,
            pending_ops: VecDeque::new(),
            control_stack: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
            status: if fiber.status == FiberStatus::Returned {
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
        Ok(Self {
            program,
            fiber,
            facade_fiber,
            entry_bound: false,
            active_dialogue: None,
            active_choice: None,
            pending_host_call: None,
            started_tasks: BTreeSet::new(),
            emitted_content: BTreeSet::new(),
            source_sequences: BTreeMap::new(),
            stream_sequences: BTreeMap::new(),
            child_fibers: VecDeque::new(),
            next_generation: 1,
            next_host_call_sequence: 0,
            compact_pure_stats: crate::step::RuntimePureCallStats::default(),
        })
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

        self.facade_fiber.env.bind_all_root_ref(root_bindings);
        self.facade_fiber.env.bind_all_root_ref(&input.bindings);
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
            if let Err(error) = self
                .fiber
                .bind_entry_arguments(&self.program, &entry_bindings)
            {
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

        let task_events = normalize_task_events(std::mem::take(&mut input.task_events));
        let source_events = normalize_source_events(std::mem::take(&mut input.source_events));
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
            if let Some(suspension) = self.fiber.suspension.clone() {
                if let Err(error) = self.fiber.resume_at(&self.program, suspension.resume) {
                    self.fail_with_error(
                        ProductStepError::Internal(error.to_string()),
                        &mut output,
                    );
                } else {
                    self.fiber.replenish_budget();
                }
            }
        } else if self.fiber.status == FiberStatus::Running {
            self.fiber.replenish_budget();
        }

        let max_ops = options.budget.max_ops;
        while executed_ops < max_ops && self.has_attemptable_work() {
            if self.fiber.status == FiberStatus::Suspended {
                let progressed =
                    self.resume_main_suspension(&input, &task_events, &mut output, pure_backend);
                executed_ops = executed_ops.saturating_add(usize::from(progressed));
                if !progressed || self.should_return_to_host(options.mode, &output, executed_ops) {
                    break;
                }
                continue;
            }

            if self.fiber.status == FiberStatus::Running {
                let step = self.step_main_vm(&mut output, pure_backend);
                executed_ops = executed_ops.saturating_add(step);
            } else if !self.step_next_child(&mut output, pure_backend) {
                break;
            } else {
                executed_ops = executed_ops.saturating_add(1);
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
            source_events_in,
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
            FiberStatus::Returned | FiberStatus::Trapped
        ) || self
            .child_fibers
            .iter()
            .any(|fiber| fiber.status == FiberStatus::Running)
    }

    fn step_main_vm(
        &mut self,
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
                    VmExit::Suspended(_) => self.initialize_suspension(output, pure_backend),
                    VmExit::Returned(value) => Self::record_return(value.as_ref(), output),
                    VmExit::Running | VmExit::Trapped(_) | VmExit::BudgetYield(_) => {}
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
                        VmExit::Returned(_) => return,
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
                    FiberStatus::Returned => {}
                    FiberStatus::Trapped => {
                        if let Some(FiberTerminalValue::Trapped(trap)) = child.terminal {
                            self.record_trap(&trap, output);
                            self.fiber.mark_trapped(trap);
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
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let Some(suspension) = self.fiber.suspension.clone() else {
            return;
        };
        match suspension.reason {
            FiberSuspensionReason::Dialogue {
                content,
                line_task_group,
            } => self.present_dialogue(content, line_task_group, output),
            FiberSuspensionReason::Choice { choice, .. } => {
                self.present_choice(choice, output, pure_backend);
            }
            FiberSuspensionReason::Await { task, .. } => self.ensure_await_started(&task, output),
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
        task_events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(suspension) = self.fiber.suspension.clone() else {
            return false;
        };
        match suspension.reason {
            FiberSuspensionReason::Dialogue {
                content,
                line_task_group,
            } => self.resume_dialogue(content, line_task_group, suspension.resume, input, output),
            FiberSuspensionReason::Choice {
                choice,
                destination,
            } => self.resume_choice(
                choice,
                destination,
                suspension.resume,
                input,
                output,
                pure_backend,
            ),
            FiberSuspensionReason::Await { task, binding } => {
                self.resume_await(&task, binding, suspension.resume, task_events, output)
            }
            FiberSuspensionReason::AwaitMany(state) => {
                self.resume_await_many(state, suspension.resume, task_events, output)
            }
            FiberSuspensionReason::HostCall {
                call, destination, ..
            } => self.resume_host_call(
                call,
                destination,
                suspension.resume,
                &input.host_call_results,
                output,
            ),
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
        output.flow_events.push(FlowEvent::DialogueLine {
            line: RuntimeLineId(line),
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
        if input_advances_dialogue(input, &line) {
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
            output.flow_events.push(FlowEvent::Goto {
                target: FlowRuntimeId(self.function_public_id(target)),
            });
        }
        true
    }

    fn ensure_await_started(&mut self, task: &RuntimeValue, output: &mut RuntimeStepOutput) {
        let task_id = TaskId(runtime_value_label(task));
        let Some(plan) = self.task_plan_for_id(&task_id.0) else {
            self.record_error(
                ProductStepError::Host(format!("missing AWBC task plan for `{}`", task_id.0)),
                output,
            );
            return;
        };
        let need = self.task_need_id(plan);
        if self.started_tasks.insert(task_id.clone()) {
            match task_spec(&self.program, plan, &task_id, Vec::new()) {
                Ok((_, spec)) => output.requests.tasks.push(spec),
                Err(error) => self.record_error(error, output),
            }
        }
        if !output
            .flow_events
            .iter()
            .any(|event| matches!(event, FlowEvent::AwaitStarted { task, .. } if task == &task_id))
        {
            output.flow_events.push(FlowEvent::AwaitStarted {
                need,
                task: task_id,
            });
        }
    }

    fn resume_await(
        &mut self,
        task: &RuntimeValue,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        resume: AwbcResumePointId,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let task_id = TaskId(runtime_value_label(task));
        let Some(event) = events.iter().find(|event| event.task_id == task_id) else {
            return false;
        };
        let need = self
            .task_plan_for_id(&task_id.0)
            .map_or_else(|| NeedId(task_id.0.clone()), |plan| self.task_need_id(plan));
        match &event.kind {
            TaskEventKind::Ready(value) => {
                if let Some(pattern) = binding
                    && let Err(error) = crate::awbc::vm::bind_pattern(
                        &self.program,
                        &mut self.fiber,
                        pattern,
                        value.value(),
                    )
                {
                    self.fail_with_trap(
                        AwbcTrapCode::PatternMismatch,
                        error.to_string(),
                        None,
                        output,
                    );
                    return true;
                }
                output.flow_events.push(FlowEvent::AwaitReady {
                    need,
                    value: value.clone(),
                });
                self.resume_at(resume, output)
            }
            TaskEventKind::Progress(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need,
                    progress: progress.clone(),
                });
                true
            }
            TaskEventKind::Err(error) => {
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    format!("await task {} failed: {error}", task_id.0),
                    None,
                    output,
                );
                true
            }
            TaskEventKind::Cancelled => {
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    format!("await task {} was cancelled", task_id.0),
                    None,
                    output,
                );
                true
            }
        }
    }

    fn fill_await_many(&mut self, output: &mut RuntimeStepOutput) {
        let Some((plan_id, limit, argument_count)) =
            self.fiber
                .suspension
                .as_ref()
                .and_then(|suspension| match &suspension.reason {
                    FiberSuspensionReason::AwaitMany(state) => {
                        self.program.task_plans.get(state.plan.index()).map(|plan| {
                            (
                                state.plan,
                                plan.many
                                    .as_ref()
                                    .map_or(usize::MAX, |policy| policy.limit.max(1) as usize),
                                plan.arguments.len(),
                            )
                        })
                    }
                    _ => None,
                })
        else {
            return;
        };
        let base_task = self.task_public_id(plan_id);
        let base_need = self.task_need_id(plan_id).0;
        let Some(suspension) = self.fiber.suspension.as_mut() else {
            return;
        };
        let FiberSuspensionReason::AwaitMany(state) = &mut suspension.reason else {
            return;
        };
        if state.results.len() != state.items.len() {
            state.results = vec![None; state.items.len()];
        }
        while state.in_flight.len() < limit && (state.next_index as usize) < state.items.len() {
            let index = state.next_index as usize;
            let task = TaskId(format!("{base_task}.{index}"));
            let need = NeedId(format!("{base_need}.{index}"));
            let args = match argument_count {
                0 => Vec::new(),
                1 => vec![state.items[index].clone()],
                count => {
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Input,
                        format!(
                            "await-many task `{base_task}` expects {count} arguments; item expansion supports zero or one"
                        ),
                    ));
                    return;
                }
            };
            let Ok(index_u32) = u32::try_from(index) else {
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Input,
                    format!("await-many task index {index} exceeds compact index range"),
                ));
                return;
            };
            match task_spec(&self.program, plan_id, &task, args) {
                Ok((_, mut spec)) => {
                    spec.key = TaskKey(spec.debug_label.clone());
                    output.flow_events.push(FlowEvent::AwaitStarted {
                        need: need.clone(),
                        task: task.clone(),
                    });
                    output.requests.tasks.push(spec);
                    state.in_flight.push(FiberAwaitManyInFlight {
                        index: index_u32,
                        task_id: task.0,
                        need_id: need.0,
                    });
                    state.next_index = state.next_index.saturating_add(1);
                }
                Err(error) => {
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        error.category(),
                        error.to_string(),
                    ));
                    return;
                }
            }
        }
    }

    fn resume_await_many(
        &mut self,
        mut state: crate::awbc::fiber::FiberAwaitManyState,
        resume: AwbcResumePointId,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if state.results.len() != state.items.len() {
            state.results = vec![None; state.items.len()];
        }
        let mut progressed = false;
        for event in events {
            let Some(position) = state
                .in_flight
                .iter()
                .position(|in_flight| in_flight.task_id == event.task_id.0)
            else {
                continue;
            };
            match &event.kind {
                TaskEventKind::Ready(value) => {
                    let in_flight = state.in_flight.remove(position);
                    state.results[in_flight.index as usize] = Some(value.value().clone());
                    output.flow_events.push(FlowEvent::AwaitReady {
                        need: NeedId(in_flight.need_id),
                        value: value.clone(),
                    });
                    progressed = true;
                }
                TaskEventKind::Progress(progress) => {
                    output.flow_events.push(FlowEvent::AwaitProgress {
                        need: NeedId(state.in_flight[position].need_id.clone()),
                        progress: progress.clone(),
                    });
                    progressed = true;
                }
                TaskEventKind::Err(error) => {
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "await task {} at index {} failed: {error}",
                            event.task_id.0, state.in_flight[position].index
                        ),
                        None,
                        output,
                    );
                    return true;
                }
                TaskEventKind::Cancelled => {
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "await task {} at index {} was cancelled",
                            event.task_id.0, state.in_flight[position].index
                        ),
                        None,
                        output,
                    );
                    return true;
                }
            }
        }
        if state.in_flight.is_empty()
            && state.next_index as usize >= state.items.len()
            && state.results.iter().all(Option::is_some)
        {
            let values = state
                .results
                .iter()
                .filter_map(Clone::clone)
                .collect::<Vec<_>>();
            let value = runtime_sequence_from_literal_values(values);
            if let Some(pattern) = state.binding
                && let Err(error) =
                    crate::awbc::vm::bind_pattern(&self.program, &mut self.fiber, pattern, &value)
            {
                self.fail_with_trap(
                    AwbcTrapCode::PatternMismatch,
                    error.to_string(),
                    None,
                    output,
                );
                return true;
            }
            output.flow_events.push(FlowEvent::AwaitReady {
                need: self.task_need_id(state.plan),
                value: RuntimePayload::from(value),
            });
            return self.resume_at(resume, output);
        }
        if let Some(suspension) = self.fiber.suspension.as_mut() {
            suspension.reason = FiberSuspensionReason::AwaitMany(state);
        }
        self.fill_await_many(output);
        progressed || !output.requests.tasks.is_empty()
    }

    fn emit_host_call(
        &mut self,
        call: AwbcHostCallId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        if self
            .pending_host_call
            .as_ref()
            .is_some_and(|pending| pending.call == call)
        {
            return;
        }
        let Some(record) = self.program.host_calls.get(call.index()) else {
            self.record_error(
                ProductStepError::Internal(format!("missing AWBC host call {}", call.0)),
                output,
            );
            return;
        };
        let public_id = self
            .program
            .strings
            .get(record.public_id.index())
            .cloned()
            .unwrap_or_else(|| format!("awbc.host_call.{}", call.0));
        let sequence = self.next_host_call_sequence;
        self.next_host_call_sequence = self.next_host_call_sequence.saturating_add(1);
        let id = RuntimeHostCallId(if sequence == 0 {
            public_id.clone()
        } else {
            format!("{public_id}.{sequence}")
        });
        output.requests.host_calls.push(RuntimeHostCallRequest {
            id: id.clone(),
            public_id,
            capability: self
                .program
                .strings
                .get(record.capability.index())
                .cloned()
                .unwrap_or_else(|| "host".to_owned()),
            operation: self
                .program
                .strings
                .get(record.operation.index())
                .cloned()
                .unwrap_or_else(|| "call".to_owned()),
            args: args.iter().cloned().map(RuntimePayload::from).collect(),
            mode: match record.mode {
                AwbcHostCallMode::Immediate => RuntimeHostCallMode::Immediate,
                AwbcHostCallMode::Suspend => RuntimeHostCallMode::Suspend,
            },
            deterministic: record.deterministic,
        });
        self.pending_host_call = Some(PendingHostCall { call, id });
    }

    fn resume_host_call(
        &mut self,
        call: AwbcHostCallId,
        destination: Option<crate::awbc::schema::AwbcRegisterId>,
        resume: AwbcResumePointId,
        results: &[crate::step::RuntimeHostCallResult],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if self.pending_host_call.is_none() {
            let args = self
                .fiber
                .suspension
                .as_ref()
                .and_then(|suspension| match &suspension.reason {
                    FiberSuspensionReason::HostCall { args, .. } => Some(args.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            self.emit_host_call(call, &args, output);
        }
        let Some(pending) = self.pending_host_call.clone() else {
            return false;
        };
        let Some(result) = results.iter().find(|result| result.id == pending.id) else {
            return false;
        };
        match &result.outcome {
            Ok(value) => {
                if let Some(destination) = destination
                    && let Ok(frame) = self.fiber.active_frame_mut()
                    && let Err(error) = frame.set_register(destination, value.value().clone())
                {
                    self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                    return true;
                }
                self.pending_host_call = None;
                self.resume_at(resume, output)
            }
            Err(error) => {
                self.pending_host_call = None;
                self.fail_with_trap(
                    match error.kind {
                        crate::step::RuntimeHostCallErrorKind::UnsupportedCapability => {
                            AwbcTrapCode::CapabilityDenied
                        }
                        crate::step::RuntimeHostCallErrorKind::Rejected
                        | crate::step::RuntimeHostCallErrorKind::Failed => {
                            AwbcTrapCode::HostAbiMismatch
                        }
                    },
                    error.message.clone(),
                    None,
                    output,
                );
                true
            }
        }
    }

    fn consume_observations(
        &mut self,
        observations: Vec<VmObservation>,
        output: &mut RuntimeStepOutput,
    ) {
        for observation in observations {
            match observation {
                VmObservation::Instruction { .. } => {}
                VmObservation::Effect { effect, args } => self.emit_effect(effect, &args, output),
                VmObservation::EnsureContent(content) => {
                    if self.emitted_content.insert(content) {
                        match content_request(&self.program, content) {
                            Ok(request) => output.requests.ensure_content.push(request),
                            Err(error) => self.record_error(error, output),
                        }
                    }
                }
                VmObservation::TaskStarted { plan, handle, args } => {
                    let task = TaskId(runtime_value_label(&handle));
                    if self.started_tasks.insert(task.clone()) {
                        match task_spec(&self.program, plan, &task, args) {
                            Ok((need, spec)) => {
                                output
                                    .flow_events
                                    .push(FlowEvent::AwaitStarted { need, task });
                                output.requests.tasks.push(spec);
                            }
                            Err(error) => self.record_error(error, output),
                        }
                    }
                }
                VmObservation::FiberSpawned { function, args, .. } => {
                    self.spawn_child(function, &args, output);
                }
                VmObservation::StreamYield { stream, value } => {
                    let sequence = self.stream_sequences.entry(stream).or_default();
                    output.effects.stream_events.push(RuntimeStreamEvent {
                        stream: stream_id_for(&self.program, stream),
                        sequence: TaskSequence(*sequence),
                        kind: SourceEventKind::Item(RuntimePayload::from(value.clone())),
                    });
                    *sequence = sequence.saturating_add(1);
                    if let Some(state) = self
                        .facade_fiber
                        .stream_states
                        .get_mut(&stream_id_for(&self.program, stream))
                    {
                        state.push_item(RuntimePayload::from(value));
                    }
                }
                VmObservation::StreamClose(stream) => {
                    let stream_id = stream_id_for(&self.program, stream);
                    if let Some(state) = self.facade_fiber.stream_states.get_mut(&stream_id)
                        && let Some(sequence) = state.close_with_sequence()
                    {
                        output.effects.stream_events.push(RuntimeStreamEvent {
                            stream: stream_id,
                            sequence,
                            kind: SourceEventKind::End,
                        });
                    }
                }
                VmObservation::SourceYield { source, value } => {
                    let source_id = source_id_for(&self.program, source);
                    let state = self
                        .facade_fiber
                        .source_states
                        .entry(source_id.clone())
                        .or_insert_with(|| {
                            SourceRuntimeState::new(source_id, SourcePolicy::default())
                        });
                    if let Some(message) = state.push_item(RuntimePayload::from(value)) {
                        output.diagnostics.push(RuntimeDiagnostic::new(message));
                    }
                }
                VmObservation::SourceClose(source) => {
                    let id = source_id_for(&self.program, source);
                    output.requests.source_close.push(id.clone());
                    if let Some(state) = self.facade_fiber.source_states.get_mut(&id) {
                        state.close();
                    }
                }
                VmObservation::Trap(trap) => self.record_trap(&trap, output),
            }
        }
    }

    fn emit_effect(
        &mut self,
        effect: AwbcEffectPlanId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        let Some(plan) = self.program.effect_plans.get(effect.index()) else {
            self.record_error(
                ProductStepError::Internal(format!("missing AWBC effect plan {}", effect.0)),
                output,
            );
            return;
        };
        match plan.kind.map_product_effect(&self.program, effect, args) {
            MappedEffect::Line(effect) => output.effects.line.push(effect),
            MappedEffect::Unsupported(diagnostic) => output.diagnostics.push(diagnostic),
        }
    }

    fn spawn_child(
        &mut self,
        function: AwbcFunctionId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        match FiberState::for_function(
            &self.program,
            self.fiber.entry,
            function,
            self.next_generation,
            self.fiber.budget.quantum.max(1),
        ) {
            Ok(mut child) => {
                self.next_generation = self.next_generation.saturating_add(1);
                match child
                    .active_frame_mut()
                    .and_then(|frame| frame.bind_positional_arguments(&self.program, args))
                {
                    Ok(()) => self.child_fibers.push_back(child),
                    Err(error) => {
                        self.record_error(ProductStepError::Type(error.to_string()), output);
                    }
                }
            }
            Err(error) => self.record_error(ProductStepError::Internal(error.to_string()), output),
        }
    }

    fn apply_source_events(
        &mut self,
        events: Vec<RuntimeSourceEvent>,
        output: &mut RuntimeStepOutput,
    ) {
        for event in events {
            if self
                .source_sequences
                .get(&event.source)
                .is_some_and(|last| event.sequence <= *last)
            {
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Input,
                    format!(
                        "stale source {} sequence {}",
                        event.source.0, event.sequence.0
                    ),
                ));
                continue;
            }
            self.source_sequences
                .insert(event.source.clone(), event.sequence);
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

    fn record_source_event_state(
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

    fn apply_unhandled_source_event(
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

    fn sync_compact_source_state(&mut self, plan: AwbcSourcePlanId) {
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

    fn spawn_source_handler(
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

    fn resume_at(&mut self, resume: AwbcResumePointId, output: &mut RuntimeStepOutput) -> bool {
        match self.fiber.resume_at(&self.program, resume) {
            Ok(()) => true,
            Err(error) => {
                self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                false
            }
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn fail_with_error(&mut self, error: ProductStepError, output: &mut RuntimeStepOutput) {
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

    fn fail_with_trap(
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
    fn record_error(&self, error: ProductStepError, output: &mut RuntimeStepOutput) {
        output.diagnostics.push(RuntimeDiagnostic::categorized(
            error.category(),
            error.to_string(),
        ));
    }

    fn record_trap(&self, trap: &FiberTrap, output: &mut RuntimeStepOutput) {
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

    fn stop_reason(
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

    fn should_return_to_host(
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

    fn sync_facade(&mut self) {
        if let Ok(frame) = self.fiber.active_frame()
            && let Some(layout) = self.program.frame_layouts.get(frame.layout.index())
        {
            for (index, slot) in layout.slots.iter().enumerate() {
                let Some(name) = slot
                    .name
                    .and_then(|id| self.program.strings.get(id.index()))
                else {
                    continue;
                };
                if let Some(value) = frame.registers.get(index).and_then(Option::as_ref) {
                    self.facade_fiber.env.set_root(name.clone(), value.clone());
                }
            }
        }
        self.facade_fiber.line_cursor =
            usize::try_from(self.fiber.line_cursor).unwrap_or(usize::MAX);
        self.facade_fiber.status = self.effective_status();
    }

    fn effective_status(&self) -> FlowFiberStatus {
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

    fn suspension_status(&self) -> FlowFiberStatus {
        let Some(suspension) = self.fiber.suspension.as_ref() else {
            return FlowFiberStatus::Running;
        };
        match &suspension.reason {
            FiberSuspensionReason::Dialogue {
                content,
                line_task_group,
            } => FlowFiberStatus::Dialogue(DialogueState {
                line: RuntimeLineId(self.content_public_id(*content)),
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
            FiberSuspensionReason::HostCall { .. } | FiberSuspensionReason::BudgetYield => {
                FlowFiberStatus::Running
            }
        }
    }

    fn choice_runtime_option(
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
            target: option
                .target
                .map(|target| FlowRuntimeId(self.function_public_id(target))),
            out,
            effects,
        }
    }

    fn content_public_id(&self, content: AwbcContentUnitId) -> String {
        self.program
            .content_units
            .get(content.index())
            .and_then(|content| self.program.strings.get(content.public_id.index()))
            .cloned()
            .unwrap_or_else(|| format!("awbc.content.{}", content.0))
    }

    fn function_public_id(&self, function: AwbcFunctionId) -> String {
        self.program
            .functions
            .get(function.index())
            .and_then(|function| function.public_id)
            .and_then(|id| self.program.strings.get(id.index()))
            .cloned()
            .unwrap_or_else(|| format!("awbc.function.{}", function.0))
    }

    fn task_plan_for_id(&self, task: &str) -> Option<AwbcTaskPlanId> {
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

    fn task_need_id(&self, plan: AwbcTaskPlanId) -> NeedId {
        NeedId(
            self.program
                .task_plans
                .get(plan.index())
                .and_then(|plan| self.program.strings.get(plan.need_id.index()))
                .cloned()
                .unwrap_or_else(|| format!("awbc.need.{}", plan.0)),
        )
    }

    fn source_plan_for_id(&self, id: &SourceId) -> Option<AwbcSourcePlanId> {
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

struct ProductVmHost<'a, B> {
    backend: &'a mut B,
    fallback_stats: &'a mut crate::step::RuntimePureCallStats,
}

impl<B: RuntimeCallBackend> VmHost for ProductVmHost<'_, B> {
    fn call_intrinsic(
        &mut self,
        program: &AwbcProgram,
        intrinsic: crate::awbc::schema::AwbcIntrinsicId,
        args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError> {
        let record = program
            .intrinsics
            .get(intrinsic.index())
            .ok_or(VmError::MissingIntrinsic(intrinsic))?;
        let name = program
            .strings
            .get(record.public_id.index())
            .ok_or(VmError::MissingString(record.public_id))?;
        let target = RuntimeCallTarget::from_label(name.clone());
        Ok(Some(crate::engine::evaluate_runtime_call(
            &target,
            args,
            self.backend,
        )))
    }

    fn call_pure_helper(
        &mut self,
        program: &AwbcProgram,
        helper: crate::awbc::schema::AwbcPureHelperId,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, VmError> {
        let record = program
            .pure_helpers
            .get(helper.index())
            .cloned()
            .ok_or_else(|| VmError::Runtime(format!("missing AWBC pure helper {}", helper.0)))?;
        let name = program
            .strings
            .get(record.public_id.index())
            .cloned()
            .ok_or(VmError::MissingString(record.public_id))?;
        let descriptor = RuntimeCompactPureHelper {
            id: helper.0,
            name,
            arity: args.len(),
            scalar_eval_supported: record.scalar_eval_supported,
        };
        if let Some(result) = self.backend.call_compact_values(&descriptor, args) {
            return result.map_err(|error| VmError::Runtime(error.to_string()));
        }
        self.fallback_stats.pure_calls = self.fallback_stats.pure_calls.saturating_add(1);
        self.fallback_stats.vm_calls = self.fallback_stats.vm_calls.saturating_add(1);
        self.fallback_stats.fallbacks = self.fallback_stats.fallbacks.saturating_add(1);
        run_function_with_host(program, record.function, args, self)
    }
}

fn run_function(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    args: &[RuntimeValue],
    backend: &mut impl RuntimeCallBackend,
    fallback_stats: &mut crate::step::RuntimePureCallStats,
) -> Result<RuntimeValue, VmError> {
    let mut host = ProductVmHost {
        backend,
        fallback_stats,
    };
    run_function_with_host(program, function, args, &mut host)
}

fn run_function_with_host(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    args: &[RuntimeValue],
    host: &mut impl VmHost,
) -> Result<RuntimeValue, VmError> {
    let mut fiber = FiberState::for_function(program, AwbcEntryId(0), function, 0, 1_000_000)?;
    fiber
        .active_frame_mut()?
        .bind_positional_arguments(program, args)?;
    loop {
        let output = step_with_host(
            program,
            &mut fiber,
            VmStepOptions {
                max_instructions: 1024,
            },
            host,
        )?;
        match output.exit {
            VmExit::Running => {}
            VmExit::Returned(value) => return Ok(value.unwrap_or(RuntimeValue::Unit)),
            VmExit::Trapped(trap) => {
                return Err(VmError::Runtime(trap.message.unwrap_or_else(|| {
                    format!("AWBC pure helper trap {:?}", trap.code)
                })));
            }
            VmExit::Suspended(reason) => {
                return Err(VmError::Runtime(format!(
                    "pure helper suspended at {reason:?}"
                )));
            }
            VmExit::BudgetYield(_) => {
                return Err(VmError::Runtime(
                    "pure helper exhausted compact VM budget".to_owned(),
                ));
            }
        }
    }
}

impl crate::awbc::schema::AwbcSourcePolicy {
    fn runtime_policy(&self) -> SourcePolicy {
        SourcePolicy {
            backpressure: match self.backpressure {
                AwbcBackpressurePolicy::LatestOnly => BackpressurePolicy::LatestOnly,
                AwbcBackpressurePolicy::BoundedQueue { capacity, overflow } => {
                    BackpressurePolicy::BoundedQueue {
                        capacity: capacity as usize,
                        on_overflow: match overflow {
                            AwbcOverflowPolicy::DropOldest => OverflowPolicy::DropOldest,
                            AwbcOverflowPolicy::DropNewest => OverflowPolicy::DropNewest,
                            AwbcOverflowPolicy::Error => OverflowPolicy::Error,
                            AwbcOverflowPolicy::Coalesce => OverflowPolicy::Coalesce,
                        },
                    }
                }
                AwbcBackpressurePolicy::BlockingNotAllowed => {
                    BackpressurePolicy::BlockingNotAllowed
                }
            },
            replay: match self.replay {
                AwbcReplayPolicy::Full => ReplayPolicy::Full,
                AwbcReplayPolicy::HashOnly => ReplayPolicy::HashOnly,
                AwbcReplayPolicy::Summary => ReplayPolicy::Summary,
                AwbcReplayPolicy::EventOnly => ReplayPolicy::EventOnly,
                AwbcReplayPolicy::None => ReplayPolicy::None,
            },
            privacy: match self.privacy {
                AwbcPrivacyPolicy::Transient => PrivacyPolicy::Transient,
                AwbcPrivacyPolicy::Redacted => PrivacyPolicy::Redacted,
                AwbcPrivacyPolicy::Recordable => PrivacyPolicy::Recordable,
                AwbcPrivacyPolicy::Private => PrivacyPolicy::Private,
            },
            max_queue: self.max_queue as usize,
        }
    }
}

fn source_id_for(program: &AwbcProgram, source: AwbcSourcePlanId) -> SourceId {
    program
        .source_plans
        .get(source.index())
        .and_then(|plan| program.strings.get(plan.public_id.index()))
        .cloned()
        .map_or_else(|| SourceId(format!("awbc.source.{}", source.0)), SourceId)
}

fn stream_id_for(program: &AwbcProgram, stream: AwbcStreamPlanId) -> StreamRuntimeId {
    program
        .stream_plans
        .get(stream.index())
        .and_then(|plan| program.strings.get(plan.public_id.index()))
        .cloned()
        .map_or_else(
            || StreamRuntimeId(format!("awbc.stream.{}", stream.0)),
            StreamRuntimeId,
        )
}

fn entry_argument_diagnostic(error: &FiberStateError) -> RuntimeDiagnostic {
    let category = match error {
        FiberStateError::EntryArgumentType { .. } => RuntimeDiagnosticCategory::Type,
        FiberStateError::EntryArgumentCount { .. }
        | FiberStateError::DuplicateEntryArgument { .. }
        | FiberStateError::UnknownEntryArgument { .. } => RuntimeDiagnosticCategory::Input,
        _ => RuntimeDiagnosticCategory::Internal,
    };
    RuntimeDiagnostic::categorized(category, error.to_string())
}

fn input_advances_dialogue(input: &RuntimeStepInput, line: &str) -> bool {
    input.input_events.iter().any(|event| {
        matches!(
            input_event_trigger_name(event),
            Some("advance" | "dialogue.advance")
        ) && input_event_text_payload(event).is_none_or(|value| value == line)
    })
}

fn input_choice_selection(input: &RuntimeStepInput) -> Option<(Option<&str>, &str)> {
    input.input_events.iter().find_map(|event| {
        let trigger = input_event_trigger_name(event)?;
        let selection = input_event_text_payload(event)?;
        match trigger {
            "choice" | "select" => Some((None, selection)),
            trigger => trigger
                .strip_prefix("choice:")
                .or_else(|| trigger.strip_prefix("select:"))
                .map(|choice| (Some(choice), selection)),
        }
    })
}

fn has_host_requests(output: &RuntimeStepOutput) -> bool {
    !output.requests.tasks.is_empty()
        || !output.requests.audio.is_empty()
        || !output.requests.cancel_scopes.is_empty()
        || !output.requests.source_close.is_empty()
        || !output.requests.ensure_content.is_empty()
        || !output.requests.host_calls.is_empty()
}

fn has_visible_output(output: &RuntimeStepOutput) -> bool {
    !output.flow_events.is_empty()
        || !output.effects.line.is_empty()
        || !output.effects.source_events.is_empty()
        || !output.effects.stream_events.is_empty()
}

#[cfg(test)]
mod tests;
