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
    input_choice_selection, run_function, stream_id_for,
};
use self::mapping::{MappedEffect, content_request, source_diagnostic, task_spec};
use self::runtime_id::line_id_from_awbc_public_id;
pub use self::snapshot::{
    AwbcProductActiveChoiceSnapshot, AwbcProductActiveDialogueSaveSnapshot,
    AwbcProductActiveDialogueSnapshot, AwbcProductChildFiberOwnerSnapshot,
    AwbcProductChildFiberSaveSnapshot, AwbcProductChildFiberSnapshot,
    AwbcProductExecutorSaveSnapshot, AwbcProductExecutorSnapshot,
    AwbcProductLineTaskCancelSnapshot, AwbcProductLineTaskExitPolicySnapshot,
    AwbcProductLineTaskExitSnapshot, AwbcProductLineTaskFiberPhaseSnapshot,
    AwbcProductLineTaskJoinSnapshot, AwbcProductLineTaskLiveSnapshot,
    AwbcProductLineTaskNodeStateSnapshot, AwbcProductLineTaskPhaseSnapshot,
    AwbcProductLineTaskWorkSnapshot, AwbcProductLineTaskWorkTagSnapshot,
    AwbcProductPendingHostCallSnapshot, AwbcProductTaskEventKindSaveSnapshot,
    AwbcProductTaskEventSaveSnapshot,
};
use crate::awbc::fiber::{
    FiberAwaitManyInFlight, FiberAwaitManyState, FiberAwaitTarget, FiberBudget, FiberCursor,
    FiberState, FiberStatus, FiberSuspensionReason, FiberTerminalValue, FiberTrap,
};
use crate::awbc::schema::{
    AwbcAwaitObserverResume, AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcEffectPlanId,
    AwbcEntryId, AwbcFunctionId, AwbcHostCallId, AwbcHostCallMode, AwbcLineTaskGroupId,
    AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger, AwbcProgram, AwbcResumePointId,
    AwbcStreamPlanId, AwbcTaskPlanId, AwbcTrapCode,
};
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::awbc::vm::{VmExit, VmObservation, VmStepOptions, step_with_host};
use crate::engine::{
    AwaitState, ChoiceState, DialogueState, FlowExit, FlowFiber, FlowFiberId, FlowFiberOwner,
    FlowFiberStatus, HostCallState,
};
use crate::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineTaskCommand, LineTaskExitPolicy, LineTaskLiveState,
    LineTaskNodeView, LineTaskPlanView, LineTaskTrigger, LineTaskWork, LineTaskWorkTag, ScopeExit,
    cancel_live_line_task_group, complete_live_line_task_work, finish_live_line_task_group,
    progress_live_line_task_group,
};
use crate::observation::RuntimeObservationState;
use crate::plan::{ChoiceRuntimeOption, FlowEvent, RuntimeHostCallTarget};
use crate::pure::{RuntimeCallBackend, VmRuntimePureCallBackend};
use crate::root::RootRuntime;
use crate::step::{
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode,
    RuntimeHostCallRequest, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
    RuntimeStepOutput, RuntimeStepResult, RuntimeStepStats, RuntimeStepStopReason,
    input_event_trigger_name,
};
use crate::stream::{RuntimeStreamEvent, StreamRuntimeState};
use crate::task::{
    AwaitTarget, HostTaskRequestTemplate, NeedId, RuntimeNeedState, TaskEvent, TaskEventKind,
    TaskId, TaskKey, TaskPublicationCursor, TaskSequence, normalize_runtime_need_states,
    normalize_task_events, resolved_runtime_need_state,
};
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeBinding, RuntimeEnv, RuntimePayload, RuntimeValue, runtime_sequence_values,
    runtime_value_label,
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
    captures: Box<[RuntimeValue]>,
    line_task: Option<LineTaskLiveState>,
    elapsed_nanos: u64,
}

impl AwbcProductStepExecutor {
    fn dialogue_group(&self, content: AwbcContentUnitId) -> Option<AwbcLineTaskGroupId> {
        self.program
            .content_units
            .get(content.index())
            .and_then(|content| content.line_task_group)
    }
}

/// Ownership of a compact child fiber. A child may never outlive an active
/// dialogue scope merely because it was stored in a shared queue.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ProductChildFiberOwner {
    Independent,
    LineTask {
        content: AwbcContentUnitId,
        tag: LineTaskWorkTag,
        policy: LineTaskExitPolicy,
        phase: ProductLineTaskFiberPhase,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProductLineTaskFiberPhase {
    Active,
    Closing,
}

#[derive(Clone, Debug, PartialEq)]
struct ProductChildFiber {
    owner: ProductChildFiberOwner,
    fiber: FiberState,
}

/// AWBC's payload-free view of one content-owned line task graph. The common
/// reducer sees only dense local node identities; AWBC function payloads are
/// resolved separately at the executor boundary.
struct AwbcLineTaskPlanView<'a> {
    program: &'a AwbcProgram,
    group: &'a crate::awbc::schema::AwbcLineTaskGroup,
    children: Vec<Box<[crate::runtime_id::RuntimeLineTaskNodeId]>>,
}

impl<'a> AwbcLineTaskPlanView<'a> {
    fn new(
        program: &'a AwbcProgram,
        group: &'a crate::awbc::schema::AwbcLineTaskGroup,
    ) -> Option<Self> {
        let end = group.nodes.checked_end()?;
        let local = |node: AwbcLineTaskNodeId| {
            node.0.checked_sub(group.nodes.start).and_then(|index| {
                crate::runtime_id::RuntimeLineTaskNodeId::from_zero_based(index as usize)
            })
        };
        let children = (group.nodes.start..end)
            .map(|index| {
                let node = program.line_task_nodes.get(index as usize)?;
                let children = match node {
                    AwbcLineTaskNode::Sequence(children)
                    | AwbcLineTaskNode::Start(children)
                    | AwbcLineTaskNode::Parallel { children, .. } => children
                        .iter()
                        .copied()
                        .map(local)
                        .collect::<Option<Vec<_>>>()?
                        .into_boxed_slice(),
                    AwbcLineTaskNode::Child { scope, .. } => {
                        vec![local(*scope)?].into_boxed_slice()
                    }
                    AwbcLineTaskNode::Action(_) => Box::default(),
                };
                Some(children)
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            program,
            group,
            children,
        })
    }

    fn global_node(
        &self,
        node: crate::runtime_id::RuntimeLineTaskNodeId,
    ) -> Option<AwbcLineTaskNodeId> {
        let offset = u32::try_from(node.index()).ok()?;
        let index = self.group.nodes.start.checked_add(offset)?;
        (index < self.group.nodes.checked_end()?).then_some(AwbcLineTaskNodeId(index))
    }

    fn global_node_to_local(
        &self,
        node: AwbcLineTaskNodeId,
    ) -> Option<crate::runtime_id::RuntimeLineTaskNodeId> {
        node.0
            .checked_sub(self.group.nodes.start)
            .and_then(|index| {
                crate::runtime_id::RuntimeLineTaskNodeId::from_zero_based(index as usize)
            })
    }

    fn function_for(&self, tag: LineTaskWorkTag) -> Option<AwbcFunctionId> {
        match tag.work {
            LineTaskWork::Node(node) => match self
                .program
                .line_task_nodes
                .get(self.global_node(node)?.index())?
            {
                AwbcLineTaskNode::Action(function) => Some(*function),
                _ => None,
            },
            LineTaskWork::Cancellation(mark) => self
                .group
                .cancel_handlers
                .iter()
                .find(|handler| handler.trigger == mark)
                .map(|handler| handler.function),
            LineTaskWork::Cleanup(ScopeExit::Completed) => self.group.cleanup_completed,
            LineTaskWork::Cleanup(ScopeExit::Cancelled) => self.group.cleanup_cancelled,
            LineTaskWork::Cleanup(ScopeExit::Failed) => self.group.cleanup_failed,
        }
    }
}

impl LineTaskPlanView for AwbcLineTaskPlanView<'_> {
    fn node_count(&self) -> usize {
        self.children.len()
    }

    fn root_node(&self) -> crate::runtime_id::RuntimeLineTaskNodeId {
        self.global_node_to_local(self.group.root)
            .expect("verified AWBC line task root belongs to its group")
    }

    fn node_view(
        &self,
        id: crate::runtime_id::RuntimeLineTaskNodeId,
    ) -> Option<LineTaskNodeView<'_>> {
        let global = self.global_node(id)?;
        let children = self.children.get(id.index())?;
        match self.program.line_task_nodes.get(global.index())? {
            AwbcLineTaskNode::Sequence(_) => Some(LineTaskNodeView::Sequence(children)),
            AwbcLineTaskNode::Start(_) => Some(LineTaskNodeView::Start(children)),
            AwbcLineTaskNode::Parallel { .. } => Some(LineTaskNodeView::Parallel(children)),
            AwbcLineTaskNode::Child {
                trigger,
                join,
                cancel,
                ..
            } => Some(LineTaskNodeView::Child {
                trigger: match trigger {
                    AwbcLineTaskTrigger::Immediate => LineTaskTrigger::Immediate,
                    AwbcLineTaskTrigger::Mark(mark) => LineTaskTrigger::Mark(*mark),
                    AwbcLineTaskTrigger::DelayNanos(nanos) => {
                        LineTaskTrigger::Delay(LogicalDuration::from_nanos(*nanos))
                    }
                },
                policy: LineTaskExitPolicy {
                    join: match join {
                        crate::awbc::schema::AwbcChildJoinPolicy::Join => ChildJoinPolicy::Join,
                        crate::awbc::schema::AwbcChildJoinPolicy::Detached => {
                            ChildJoinPolicy::Detached
                        }
                    },
                    cancel: match cancel {
                        crate::awbc::schema::AwbcChildCancelPolicy::CancelAndJoin => {
                            ChildCancelPolicy::CancelAndJoin
                        }
                        crate::awbc::schema::AwbcChildCancelPolicy::Finish => {
                            ChildCancelPolicy::Finish
                        }
                        crate::awbc::schema::AwbcChildCancelPolicy::Detach => {
                            ChildCancelPolicy::Detach
                        }
                    },
                },
                scope: *children.first()?,
            }),
            AwbcLineTaskNode::Action(_) => Some(LineTaskNodeView::Action),
        }
    }

    fn has_action(&self, node: crate::runtime_id::RuntimeLineTaskNodeId) -> bool {
        self.global_node(node)
            .and_then(|node| self.program.line_task_nodes.get(node.index()))
            .is_some_and(|node| matches!(node, AwbcLineTaskNode::Action(_)))
    }

    fn cancellation_mark(
        &self,
        marks: &BTreeSet<crate::runtime_id::RuntimeDialogueMarkId>,
    ) -> Option<crate::runtime_id::RuntimeDialogueMarkId> {
        self.group
            .cancel_handlers
            .iter()
            .find(|handler| marks.contains(&handler.trigger))
            .map(|handler| handler.trigger)
    }

    fn has_cancellation_work(&self, mark: crate::runtime_id::RuntimeDialogueMarkId) -> bool {
        self.group
            .cancel_handlers
            .iter()
            .any(|handler| handler.trigger == mark)
    }

    fn has_cleanup(&self, exit: ScopeExit) -> bool {
        match exit {
            ScopeExit::Completed => self.group.cleanup_completed.is_some(),
            ScopeExit::Cancelled => self.group.cleanup_cancelled.is_some(),
            ScopeExit::Failed => self.group.cleanup_failed.is_some(),
        }
    }
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

/// Product-only presentation state derived from the compact fiber.
/// Evaluated await-many state remains in AWBC coordinates and never becomes a
/// synthetic plan-qualified expression.
#[derive(Clone, Debug, PartialEq)]
enum AwbcProductExecutorStatus {
    Shared(FlowFiberStatus),
    WaitingMany(FiberAwaitManyState),
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
    task_publications: BTreeMap<TaskId, TaskPublicationCursor>,
    need_publications: BTreeMap<NeedId, TaskPublicationCursor>,
    queued_task_events: VecDeque<TaskEvent>,
    emitted_content: BTreeSet<AwbcContentUnitId>,
    stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    child_fibers: VecDeque<ProductChildFiber>,
    next_line_task_activation: u64,
    next_generation: u64,
    next_host_call_sequence: u64,
    next_audio_sequence: u64,
    compact_pure_stats: crate::step::RuntimePureCallStats,
    root: Option<RootRuntime>,
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
            await_observer: None,
            root_cleanups: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            stream_states: BTreeMap::new(),
            id: FlowFiberId::from_executor_ordinal(0),
            owner: FlowFiberOwner::Executor,
            status: if matches!(fiber.status, FiberStatus::Returned | FiberStatus::Cancelled) {
                FlowFiberStatus::Done(FlowExit::Done)
            } else {
                FlowFiberStatus::Running
            },
        };
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
            task_publications: BTreeMap::new(),
            need_publications: BTreeMap::new(),
            queued_task_events: VecDeque::new(),
            emitted_content: BTreeSet::new(),
            stream_sequences: BTreeMap::new(),
            child_fibers: VecDeque::new(),
            next_line_task_activation: 0,
            next_generation: 1,
            next_host_call_sequence: 0,
            next_audio_sequence: 0,
            compact_pure_stats: crate::step::RuntimePureCallStats::default(),
            root: None,
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
        let need_states_in = need_states.len();
        let task_events_in = task_events.len();
        output.diagnostics.extend(task_events.iter().map(|event| {
            RuntimeDiagnostic::new(format!(
                "task {} sequence {} delivered",
                event.task_id.0, event.sequence.0
            ))
        }));
        self.latch_task_events(&task_events);
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
            root_events_in,
            root_transitions: output.root_transitions.len(),
            root_commands: output.root_commands.len(),
            root_events_deferred: output.requests.root_events_next_step.len(),
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
            .any(|child| child.fiber.status == FiberStatus::Running)
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
        let owner = child.owner.clone();
        if child.fiber.status != FiberStatus::Running {
            self.child_fibers.push_back(child);
            return false;
        }
        child.fiber.replenish_budget();
        let mut host = ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut self.compact_pure_stats,
        };
        let mut completed = None;
        match step_with_host(
            &self.program,
            &mut child.fiber,
            VmStepOptions {
                max_instructions: 1,
            },
            &mut host,
        ) {
            Ok(vm_output) => {
                self.consume_observations(vm_output.observations, output);
                match child.fiber.status {
                    FiberStatus::Running => {
                        self.child_fibers.push_back(child);
                    }
                    FiberStatus::Suspended => {
                        if matches!(&owner, ProductChildFiberOwner::LineTask { .. }) {
                            completed = Some(true);
                            self.record_error(
                                ProductStepError::Internal(
                                    "AWBC line-task action suspended without an owned resume protocol"
                                        .to_owned(),
                                ),
                                output,
                            );
                        } else {
                            self.child_fibers.push_back(child);
                        }
                    }
                    FiberStatus::Returned | FiberStatus::Cancelled => {
                        completed = Some(false);
                    }
                    FiberStatus::Trapped => {
                        completed = Some(true);
                        if let Some(FiberTerminalValue::Trapped(trap)) = child.fiber.terminal {
                            self.terminate_with_trap(trap, output);
                        }
                    }
                }
            }
            Err(error) => {
                completed = Some(true);
                self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
            }
        }
        if let (
            Some(failed),
            ProductChildFiberOwner::LineTask {
                content,
                tag,
                policy,
                ..
            },
        ) = (completed, owner)
            && policy.join == ChildJoinPolicy::Join
        {
            self.complete_owned_line_task_work(content, tag, failed, output);
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
                values,
                line_task_captures,
            } => self.present_dialogue(content, values, line_task_captures, output),
            FiberSuspensionReason::Choice { choice, .. } => {
                self.present_choice(choice, output, pure_backend);
            }
            FiberSuspensionReason::Await {
                target,
                binding,
                observer,
            } => match target {
                FiberAwaitTarget::Task(task) => self.ensure_await_started(&task, output),
                FiberAwaitTarget::Need(need) => {
                    if let Some(resume) = declared_resume {
                        self.resume_need(&need, binding, observer, resume, need_states, output);
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
                values,
                line_task_captures,
            } => self.resume_dialogue(content, values, line_task_captures, resume, input, output),
            FiberSuspensionReason::Choice {
                choice,
                destination,
            } => self.resume_choice(choice, destination, resume, input, output, pure_backend),
            FiberSuspensionReason::Await {
                target,
                binding,
                observer,
            } => match target {
                FiberAwaitTarget::Task(task) => {
                    self.resume_await(&task, binding, observer, resume, task_events, output)
                }
                FiberAwaitTarget::Need(need) => {
                    self.resume_need(&need, binding, observer, resume, need_states, output)
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
        values: Box<[crate::plan::RuntimeDialogueValueBinding]>,
        captures: Box<[RuntimeValue]>,
        output: &mut RuntimeStepOutput,
    ) {
        if self
            .active_dialogue
            .as_ref()
            .is_some_and(|active| active.content == content)
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
            values,
        });
        self.emitted_content.insert(content);
        let line_task = self.line_task_state(content);
        let mut active = ActiveDialogue {
            content,
            captures,
            line_task,
            elapsed_nanos: 0,
        };
        self.progress_line_task(&mut active, &BTreeSet::new(), output);
        self.active_dialogue = Some(active);
        self.fiber.line_cursor = self.fiber.line_cursor.saturating_add(1);
    }

    fn resume_dialogue(
        &mut self,
        content: AwbcContentUnitId,
        values: Box<[crate::plan::RuntimeDialogueValueBinding]>,
        captures: Box<[RuntimeValue]>,
        resume: AwbcResumePointId,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if self.active_dialogue.is_none() {
            self.present_dialogue(content, values, captures.clone(), output);
        }
        let mut active = self.active_dialogue.take().unwrap_or(ActiveDialogue {
            content,
            captures,
            line_task: self.line_task_state(content),
            elapsed_nanos: 0,
        });
        active.elapsed_nanos = active.elapsed_nanos.saturating_add(input.dt.as_nanos());
        let marks = self.dialogue_marks(active.content, input);
        self.progress_line_task(&mut active, &marks, output);
        if let Some(trigger) = self.cancel_line_task(&mut active, &marks, output) {
            output.flow_events.push(FlowEvent::LineCancelled {
                trigger: self.dialogue_mark_label(content, trigger),
            });
        }
        let line = self.content_public_id(content);
        if input.advances_dialogue_label(&line) {
            self.finish_line_task(&mut active, output);
            if active
                .line_task
                .as_ref()
                .is_none_or(LineTaskLiveState::is_closed)
            {
                self.active_dialogue = None;
                return self.resume_at(resume, output);
            }
        }
        self.active_dialogue = Some(active);
        false
    }

    fn line_task_view(&self, content: AwbcContentUnitId) -> Option<AwbcLineTaskPlanView<'_>> {
        let group = self
            .dialogue_group(content)
            .and_then(|group| self.program.line_task_groups.get(group.index()))?;
        AwbcLineTaskPlanView::new(&self.program, group)
    }

    fn line_task_state(&mut self, content: AwbcContentUnitId) -> Option<LineTaskLiveState> {
        let group = self.dialogue_group(content)?;
        self.program.line_task_groups.get(group.index())?;
        let activation = self.next_line_task_activation;
        self.next_line_task_activation = self.next_line_task_activation.saturating_add(1);
        let view = self.line_task_view(content)?;
        Some(LineTaskLiveState::new(&view, activation))
    }

    fn progress_line_task(
        &mut self,
        active: &mut ActiveDialogue,
        marks: &BTreeSet<crate::runtime_id::RuntimeDialogueMarkId>,
        output: &mut RuntimeStepOutput,
    ) {
        let commands = {
            let Some(view) = self.line_task_view(active.content) else {
                self.record_error(
                    ProductStepError::Internal(
                        "dialogue content has no verified AWBC line-task view".to_owned(),
                    ),
                    output,
                );
                return;
            };
            let Some(state) = active.line_task.as_mut() else {
                return;
            };
            progress_live_line_task_group(
                &view,
                LogicalDuration::from_nanos(active.elapsed_nanos),
                marks,
                state,
            )
            .commands
        };
        self.execute_line_task_commands(active, commands, output);
    }

    fn cancel_line_task(
        &mut self,
        active: &mut ActiveDialogue,
        marks: &BTreeSet<crate::runtime_id::RuntimeDialogueMarkId>,
        output: &mut RuntimeStepOutput,
    ) -> Option<crate::runtime_id::RuntimeDialogueMarkId> {
        let (selected, commands) = {
            let view = self.line_task_view(active.content)?;
            let state = active.line_task.as_mut()?;
            let selected = view.cancellation_mark(marks);
            let activation = cancel_live_line_task_group(&view, marks, state);
            let selected = activation.as_ref().and(selected);
            (selected, activation.unwrap_or_default().commands)
        };
        self.execute_line_task_commands(active, commands, output);
        selected
    }

    fn finish_line_task(&mut self, active: &mut ActiveDialogue, output: &mut RuntimeStepOutput) {
        let commands = {
            let Some(view) = self.line_task_view(active.content) else {
                return;
            };
            let Some(state) = active.line_task.as_mut() else {
                return;
            };
            finish_live_line_task_group(&view, state).commands
        };
        self.execute_line_task_commands(active, commands, output);
    }

    fn execute_line_task_commands(
        &mut self,
        active: &mut ActiveDialogue,
        commands: Vec<LineTaskCommand>,
        output: &mut RuntimeStepOutput,
    ) {
        let commands = {
            let Some(view) = self.line_task_view(active.content) else {
                return;
            };
            commands
                .into_iter()
                .map(|command| {
                    let function = match &command {
                        LineTaskCommand::Run { tag, .. } => view.function_for(*tag),
                        LineTaskCommand::Cancel { .. } => None,
                    };
                    (command, function)
                })
                .collect::<Vec<_>>()
        };
        for (command, function) in commands {
            match command {
                LineTaskCommand::Run { tag, policy } => {
                    let Some(function) = function else {
                        self.record_error(
                            ProductStepError::Internal(
                                "AWBC line-task reducer command has no action function".to_owned(),
                            ),
                            output,
                        );
                        continue;
                    };
                    let phase = if matches!(tag.work, LineTaskWork::Node(_)) {
                        ProductLineTaskFiberPhase::Active
                    } else {
                        ProductLineTaskFiberPhase::Closing
                    };
                    self.spawn_owned_child(
                        ProductChildFiberOwner::LineTask {
                            content: active.content,
                            tag,
                            policy,
                            phase,
                        },
                        function,
                        &active.captures,
                        output,
                    );
                }
                LineTaskCommand::Cancel { activation, node } => {
                    self.cancel_line_task_children(active, activation, node, output);
                }
            }
        }
    }

    fn cancel_line_task_children(
        &mut self,
        active: &mut ActiveDialogue,
        activation: crate::line_task::LineTaskActivationId,
        node: crate::runtime_id::RuntimeLineTaskNodeId,
        output: &mut RuntimeStepOutput,
    ) {
        let mut completed = Vec::new();
        let mut observations = Vec::new();
        let mut detach_rejected = false;
        self.child_fibers.retain_mut(|child| {
            let ProductChildFiberOwner::LineTask {
                content,
                tag,
                policy,
                phase,
            } = &mut child.owner
            else {
                return true;
            };
            if *content != active.content
                || tag.activation != activation
                || tag.work != LineTaskWork::Node(node)
            {
                return true;
            }
            match policy.cancel {
                ChildCancelPolicy::CancelAndJoin => {
                    observations
                        .extend(crate::awbc::vm::cancel_fiber(&mut child.fiber).observations);
                    completed.push(*tag);
                    false
                }
                ChildCancelPolicy::Finish => {
                    *phase = ProductLineTaskFiberPhase::Closing;
                    true
                }
                ChildCancelPolicy::Detach => {
                    observations
                        .extend(crate::awbc::vm::cancel_fiber(&mut child.fiber).observations);
                    detach_rejected = true;
                    completed.push(*tag);
                    false
                }
            }
        });
        self.consume_observations(observations, output);
        if detach_rejected {
            self.record_error(
                ProductStepError::Internal(
                    "AWBC line-task detach has no verified detached-owner boundary".to_owned(),
                ),
                output,
            );
        }
        for tag in completed {
            self.complete_line_task_work(active, tag, false, output);
        }
    }

    fn complete_line_task_work(
        &mut self,
        active: &mut ActiveDialogue,
        tag: LineTaskWorkTag,
        failed: bool,
        output: &mut RuntimeStepOutput,
    ) {
        let commands = {
            let Some(view) = self.line_task_view(active.content) else {
                return;
            };
            let Some(state) = active.line_task.as_mut() else {
                return;
            };
            complete_live_line_task_work(&view, state, tag, failed).commands
        };
        self.execute_line_task_commands(active, commands, output);
    }

    fn complete_owned_line_task_work(
        &mut self,
        content: AwbcContentUnitId,
        tag: LineTaskWorkTag,
        failed: bool,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(mut active) = self.active_dialogue.take() else {
            self.record_error(
                ProductStepError::Internal(
                    "AWBC line-owned child completed outside an active dialogue".to_owned(),
                ),
                output,
            );
            return;
        };
        if active.content == content {
            self.complete_line_task_work(&mut active, tag, failed, output);
        } else {
            self.record_error(
                ProductStepError::Internal(
                    "AWBC line-owned child completed for a stale dialogue content".to_owned(),
                ),
                output,
            );
        }
        self.active_dialogue = Some(active);
    }

    fn dialogue_marks(
        &self,
        content: AwbcContentUnitId,
        input: &RuntimeStepInput,
    ) -> BTreeSet<crate::runtime_id::RuntimeDialogueMarkId> {
        let Some(content) = self.program.content_units.get(content.index()) else {
            return BTreeSet::new();
        };
        content
            .marks
            .iter()
            .filter(|mark| {
                self.program
                    .strings
                    .get(mark.label.index())
                    .is_some_and(|label| {
                        input.input_events.iter().any(|event| {
                            input_event_trigger_name(event).is_some_and(|trigger| {
                                trigger == label || trigger == format!("mark:{label}")
                            })
                        })
                    })
            })
            .map(|mark| mark.id)
            .collect()
    }

    fn dialogue_mark_label(
        &self,
        content: AwbcContentUnitId,
        id: crate::runtime_id::RuntimeDialogueMarkId,
    ) -> String {
        self.program
            .content_units
            .get(content.index())
            .and_then(|content| content.marks.iter().find(|mark| mark.id == id))
            .and_then(|mark| self.program.strings.get(mark.label.index()))
            .cloned()
            .unwrap_or_else(|| id.to_string())
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
