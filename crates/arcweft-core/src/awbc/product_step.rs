//! Product AWBC runtime-step parity adapter.
//!
//! This module maps the compact AWBC VM/fiber boundary into the existing
//! `RuntimeStepResult` host surface. It remains Sans I/O; host work is emitted
//! as typed requests and product hosts decide how to fulfill those requests.

use crate::awbc::fiber::{
    FiberBudget, FiberCursor, FiberState, FiberStatus, FiberSuspensionReason, FiberTerminalValue,
};
use crate::awbc::schema::{
    AwbcBlockId, AwbcChoiceId, AwbcEffectKind, AwbcEffectPlanId, AwbcEntryId, AwbcFunctionId,
    AwbcInstruction, AwbcProgram, AwbcTaskClass, AwbcTaskPlanId, AwbcTaskPolicy, AwbcTerminator,
};
use crate::awbc::vm::{RejectingVmHost, VmExit, VmObservation, VmStepOptions, step_with_host};
use crate::effect::{LineEffectRequest, RuntimeCall, RuntimeLog};
use crate::engine::{FlowExit, FlowFiber, FlowFiberStatus};
use crate::observation::RuntimeObservationState;
use crate::plan::{ChoiceRuntimeOption, FlowEvent};
use crate::source::{SourceEventKind, SourceId};
use crate::step::{
    RuntimeDiagnostic, RuntimeStepInput, RuntimeStepOptions, RuntimeStepOutput, RuntimeStepResult,
    RuntimeStepStats, RuntimeStepStopReason,
};
use crate::stream::{RuntimeStreamEvent, StreamRuntimeId};
use crate::task::{
    CancelScopeId, HostTaskRequest, TaskId, TaskKey, TaskPolicy, TaskPriority, TaskSequence,
    TaskSpec,
};
use crate::value::{RuntimeEnv, RuntimePayload, RuntimeValue, runtime_value_label};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

/// Compact executable families that still lack product `RuntimeStepResult` parity.
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

impl AwbcInstruction {
    /// Returns the product-step parity family that blocks this instruction.
    #[must_use]
    pub const fn product_step_parity_blocker(&self) -> Option<AwbcProductStepParityBlocker> {
        match self {
            Self::CallPureHelper { .. } => Some(AwbcProductStepParityBlocker::PureHelperCalls),
            Self::CallIntrinsic { .. } => Some(AwbcProductStepParityBlocker::IntrinsicCalls),
            Self::EnsureContent { .. } => Some(AwbcProductStepParityBlocker::ContentEnsures),
            Self::EmitEffect { .. } => Some(AwbcProductStepParityBlocker::Effects),
            Self::StartTask { .. } => Some(AwbcProductStepParityBlocker::TaskStarts),
            Self::SpawnFiber { .. } => Some(AwbcProductStepParityBlocker::SpawnedFibers),
            Self::StreamYield { .. } | Self::StreamClose { .. } => {
                Some(AwbcProductStepParityBlocker::Streams)
            }
            Self::SourceClose { .. } => Some(AwbcProductStepParityBlocker::Sources),
            Self::BindPattern { .. }
            | Self::TestPattern { .. }
            | Self::RepeatSequence { .. }
            | Self::SequenceLen { .. }
            | Self::SequenceGet { .. }
            | Self::SequenceSlice { .. }
            | Self::SequencePush { .. }
            | Self::MakeRecord { .. }
            | Self::MakeVariant { .. }
            | Self::ProjectTuple { .. }
            | Self::ProjectRecord { .. }
            | Self::ProjectField { .. }
            | Self::Unary { .. }
            | Self::Binary { .. } => Some(AwbcProductStepParityBlocker::TrapSourceReporting),
            Self::Nop
            | Self::LoadConst { .. }
            | Self::Move { .. }
            | Self::Clear { .. }
            | Self::EnterScope { .. }
            | Self::ExitScope { .. }
            | Self::MakeTuple { .. }
            | Self::MakeSequence { .. }
            | Self::Drop { .. } => None,
        }
    }
}

impl AwbcTerminator {
    /// Returns the product-step parity family that blocks this terminator.
    #[must_use]
    pub const fn product_step_parity_blocker(&self) -> Option<AwbcProductStepParityBlocker> {
        match self {
            Self::Dialogue { .. } => Some(AwbcProductStepParityBlocker::Dialogue),
            Self::Choice { .. } => Some(AwbcProductStepParityBlocker::Choice),
            Self::Await { .. } => Some(AwbcProductStepParityBlocker::Await),
            Self::AwaitMany { .. } => Some(AwbcProductStepParityBlocker::AwaitMany),
            Self::HostCall { .. } => Some(AwbcProductStepParityBlocker::HostCall),
            Self::BudgetYield { .. } => Some(AwbcProductStepParityBlocker::BudgetYield),
            Self::Trap { .. }
            | Self::GotoDynamic { .. }
            | Self::Match { .. }
            | Self::Unreachable => Some(AwbcProductStepParityBlocker::TrapSourceReporting),
            Self::Jump { .. }
            | Self::Branch { .. }
            | Self::CallFunction { .. }
            | Self::GotoStatic { .. }
            | Self::Return { .. } => None,
        }
    }
}

impl AwbcProgram {
    /// Inventories unsupported product-step families in deterministic order.
    #[must_use]
    pub fn product_step_parity_blockers(&self) -> Vec<AwbcProductStepParityBlocker> {
        let mut blockers = BTreeSet::new();
        if self.entries.iter().any(|entry| {
            self.signatures
                .get(entry.signature.index())
                .is_some_and(|signature| !signature.params.is_empty())
        }) {
            blockers.insert(AwbcProductStepParityBlocker::EntryArguments);
        }
        blockers.extend(
            self.instructions
                .iter()
                .filter_map(AwbcInstruction::product_step_parity_blocker),
        );
        blockers.extend(
            self.blocks
                .iter()
                .filter_map(|block| block.terminator.product_step_parity_blocker()),
        );
        blockers.into_iter().collect()
    }

    /// Rejects a compact program before execution when parity is incomplete.
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
pub struct AwbcProductStepExecutor {
    program: AwbcProgram,
    fiber: FiberState,
    facade_fiber: FlowFiber,
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
            FiberState::for_entry(&program, entry, 0, budget_quantum).map_err(|error| {
                AwbcProductStepBuildError::FiberState {
                    message: error.to_string(),
                }
            })?
        };
        let facade_fiber = FlowFiber {
            line_cursor: 0,
            cursor: None,
            pending_ops: VecDeque::new(),
            control_stack: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
            status: if matches!(fiber.status, FiberStatus::Returned) {
                FlowFiberStatus::Done(FlowExit::Done)
            } else {
                FlowFiberStatus::Running
            },
        };
        Ok(Self {
            program,
            fiber,
            facade_fiber,
        })
    }

    pub const fn program(&self) -> &AwbcProgram {
        &self.program
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.facade_fiber
    }

    pub fn step(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        let mut output = RuntimeStepOutput::default();
        let task_events_in = input.task_events.len();
        let source_events_in = input.source_events.len();
        self.apply_external_events(input, &mut output);

        if matches!(
            self.fiber.status,
            FiberStatus::Returned | FiberStatus::Trapped
        ) {
            let status = self.status_from_fiber();
            self.facade_fiber.status = status.clone();
            let diagnostics = output.diagnostics.len();
            return RuntimeStepResult {
                output,
                fiber_status: status,
                stop_reason: if matches!(self.fiber.status, FiberStatus::Returned) {
                    RuntimeStepStopReason::Done
                } else {
                    RuntimeStepStopReason::Failed
                },
                stats: RuntimeStepStats {
                    task_events_in,
                    source_events_in,
                    diagnostics,
                    ..RuntimeStepStats::default()
                },
            };
        }

        if matches!(self.fiber.status, FiberStatus::Suspended) {
            self.try_resume_from_host_input(&mut output);
        }
        if matches!(self.fiber.status, FiberStatus::Suspended) {
            let status = self.status_from_fiber();
            self.facade_fiber.status = status.clone();
            let diagnostics = output.diagnostics.len();
            return RuntimeStepResult {
                output,
                fiber_status: status,
                stop_reason: RuntimeStepStopReason::Blocked,
                stats: RuntimeStepStats {
                    task_events_in,
                    source_events_in,
                    diagnostics,
                    ..RuntimeStepStats::default()
                },
            };
        }

        self.fiber.replenish_budget();
        let max_instructions = u64::try_from(options.budget.max_ops.max(1)).unwrap_or(u64::MAX);
        let mut host = RejectingVmHost;
        match step_with_host(
            &self.program,
            &mut self.fiber,
            VmStepOptions { max_instructions },
            &mut host,
        ) {
            Ok(vm_output) => {
                self.consume_observations(vm_output.observations, &mut output);
                let stop_reason = Self::stop_reason_for_exit(&vm_output.exit, &output);
                let status = self.status_from_exit(&vm_output.exit);
                self.facade_fiber.status = status.clone();
                RuntimeStepResult {
                    stats: RuntimeStepStats {
                        executed_ops: usize::try_from(vm_output.executed).unwrap_or(usize::MAX),
                        task_events_in,
                        source_events_in,
                        source_events_emitted: output.effects.source_events.len(),
                        stream_events_emitted: output.effects.stream_events.len(),
                        line_effects: output.effects.line.len(),
                        audio_commands: output.requests.audio.len(),
                        diagnostics: output.diagnostics.len(),
                        ..RuntimeStepStats::default()
                    },
                    output,
                    fiber_status: status,
                    stop_reason,
                }
            }
            Err(error) => {
                output.diagnostics.push(RuntimeDiagnostic {
                    message: error.to_string(),
                });
                let status = FlowFiberStatus::Failed(error.to_string());
                self.facade_fiber.status = status.clone();
                RuntimeStepResult {
                    output,
                    fiber_status: status,
                    stop_reason: RuntimeStepStopReason::Failed,
                    stats: RuntimeStepStats {
                        task_events_in,
                        source_events_in,
                        diagnostics: 1,
                        ..RuntimeStepStats::default()
                    },
                }
            }
        }
    }

    fn apply_external_events(&mut self, input: RuntimeStepInput, output: &mut RuntimeStepOutput) {
        for source_event in input.source_events {
            output.effects.source_events.push(source_event);
        }
        for task_event in input.task_events {
            if let Some(suspension) = self.fiber.suspension.clone() {
                match (suspension.reason, task_event.kind) {
                    (
                        FiberSuspensionReason::Await { binding, .. },
                        crate::task::TaskEventKind::Ready(value),
                    ) => {
                        let value = value.into_value();
                        if let Some(pattern) = binding
                            && let Err(error) = self.bind_suspended_value(pattern, &value)
                        {
                            output
                                .diagnostics
                                .push(RuntimeDiagnostic { message: error });
                        }
                        let _ = self.fiber.resume_at(&self.program, suspension.resume);
                        output.flow_events.push(FlowEvent::AwaitReady {
                            need: crate::task::NeedId(task_event.task_id.0.clone()),
                            value: RuntimePayload::from(value),
                        });
                    }
                    (
                        FiberSuspensionReason::Await { .. },
                        crate::task::TaskEventKind::Err(error),
                    ) => {
                        output
                            .diagnostics
                            .push(RuntimeDiagnostic { message: error });
                    }
                    (_, crate::task::TaskEventKind::Progress(progress)) => {
                        output.flow_events.push(FlowEvent::AwaitProgress {
                            need: crate::task::NeedId(task_event.task_id.0.clone()),
                            progress,
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    fn try_resume_from_host_input(&mut self, output: &mut RuntimeStepOutput) {
        let Some(suspension) = self.fiber.suspension.clone() else {
            return;
        };
        match suspension.reason {
            FiberSuspensionReason::Dialogue { .. } | FiberSuspensionReason::BudgetYield => {
                let _ = self.fiber.resume_at(&self.program, suspension.resume);
            }
            FiberSuspensionReason::Choice {
                choice,
                destination,
            } => {
                let Some(option) = self.first_choice_option(choice) else {
                    return;
                };
                if let Ok(frame) = self.fiber.active_frame_mut() {
                    let _ = frame.set_register(destination, RuntimeValue::String(option.clone()));
                }
                let _ = self.fiber.resume_at(&self.program, suspension.resume);
                output
                    .flow_events
                    .push(FlowEvent::ChoiceSelected { id: None, option });
            }
            FiberSuspensionReason::HostCall { .. }
            | FiberSuspensionReason::Await { .. }
            | FiberSuspensionReason::AwaitMany(_) => {}
        }
    }

    fn bind_suspended_value(
        &mut self,
        pattern: crate::awbc::schema::AwbcPatternId,
        value: &RuntimeValue,
    ) -> Result<(), String> {
        let pattern = self
            .program
            .patterns
            .get(pattern.index())
            .ok_or_else(|| "missing AWBC await pattern".to_owned())?;
        if let crate::awbc::schema::AwbcPattern::Bind { target, .. }
        | crate::awbc::schema::AwbcPattern::Whole { target, .. } = pattern
        {
            self.fiber
                .active_frame_mut()
                .map_err(|error| error.to_string())?
                .set_register(*target, value.clone())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn consume_observations(
        &self,
        observations: Vec<VmObservation>,
        output: &mut RuntimeStepOutput,
    ) {
        for observation in observations {
            match observation {
                VmObservation::Effect(effect) => {
                    if let Some(effect) = self.line_effect(effect) {
                        output.effects.line.push(effect);
                    }
                }
                VmObservation::StreamYield { stream, value } => {
                    output.effects.stream_events.push(RuntimeStreamEvent {
                        stream: self.stream_id(stream),
                        sequence: TaskSequence(0),
                        kind: SourceEventKind::Item(RuntimePayload::from(value)),
                    });
                }
                VmObservation::StreamClose(stream) => {
                    output.effects.stream_events.push(RuntimeStreamEvent {
                        stream: self.stream_id(stream),
                        sequence: TaskSequence(0),
                        kind: SourceEventKind::End,
                    });
                }
                VmObservation::SourceClose(source) => {
                    output.requests.source_close.push(self.source_id(source));
                }
                VmObservation::Trap(trap) => output.diagnostics.push(RuntimeDiagnostic {
                    message: trap
                        .message
                        .unwrap_or_else(|| format!("AWBC trap {:?}", trap.code)),
                }),
                VmObservation::EnsureContent(_) | VmObservation::Instruction { .. } => {}
            }
        }
    }

    fn stop_reason_for_exit(exit: &VmExit, output: &RuntimeStepOutput) -> RuntimeStepStopReason {
        match exit {
            VmExit::Returned(_) => RuntimeStepStopReason::Done,
            VmExit::Trapped(_) => RuntimeStepStopReason::Failed,
            VmExit::BudgetYield(_) => RuntimeStepStopReason::BudgetExhausted,
            VmExit::Suspended(_)
                if !output.flow_events.is_empty() || !output.effects.line.is_empty() =>
            {
                RuntimeStepStopReason::Output
            }
            VmExit::Suspended(_) => RuntimeStepStopReason::Blocked,
            VmExit::Running => RuntimeStepStopReason::OneOp,
        }
    }

    fn status_from_exit(&self, exit: &VmExit) -> FlowFiberStatus {
        match exit {
            VmExit::Returned(Some(value)) => {
                FlowFiberStatus::Done(FlowExit::Return(runtime_value_label(value)))
            }
            VmExit::Returned(None) => FlowFiberStatus::Done(FlowExit::Done),
            VmExit::Trapped(trap) => FlowFiberStatus::Failed(
                trap.message
                    .clone()
                    .unwrap_or_else(|| format!("AWBC trap {:?}", trap.code)),
            ),
            VmExit::Suspended(reason) => self.status_from_suspension(reason),
            VmExit::BudgetYield(_) | VmExit::Running => FlowFiberStatus::Running,
        }
    }

    fn status_from_fiber(&self) -> FlowFiberStatus {
        match self.fiber.terminal.as_ref() {
            Some(FiberTerminalValue::Returned(Some(value))) => {
                FlowFiberStatus::Done(FlowExit::Return(runtime_value_label(value)))
            }
            Some(FiberTerminalValue::Returned(None)) => FlowFiberStatus::Done(FlowExit::Done),
            Some(FiberTerminalValue::Trapped(trap)) => FlowFiberStatus::Failed(
                trap.message
                    .clone()
                    .unwrap_or_else(|| format!("AWBC trap {:?}", trap.code)),
            ),
            None => self
                .fiber
                .suspension
                .as_ref()
                .map_or(FlowFiberStatus::Running, |suspension| {
                    self.status_from_suspension(&suspension.reason)
                }),
        }
    }

    fn status_from_suspension(&self, reason: &FiberSuspensionReason) -> FlowFiberStatus {
        match reason {
            FiberSuspensionReason::Choice { choice, .. } => {
                FlowFiberStatus::Choice(crate::engine::ChoiceState {
                    id: self.choice_public_id(*choice),
                    options: self.choice_options(*choice),
                    resume: None,
                })
            }
            FiberSuspensionReason::Await { task, .. } => {
                FlowFiberStatus::Waiting(crate::engine::AwaitState {
                    binding: None,
                    target: crate::task::AwaitTarget::new(
                        crate::task::NeedId(runtime_value_label(task)),
                        crate::task::TaskId(runtime_value_label(task)),
                        crate::task::HostTaskRequestTemplate::new("awbc", "await", []),
                    ),
                    resume: None,
                })
            }
            FiberSuspensionReason::AwaitMany(state) => {
                FlowFiberStatus::WaitingMany(Box::new(crate::engine::AwaitManyState {
                    binding: None,
                    target: crate::task::AwaitManyTarget::new(
                        crate::task::NeedId(format!("awbc.await_many.{}", state.plan.0)),
                        crate::task::TaskId(format!("awbc.await_many.{}", state.plan.0)),
                        crate::value::RuntimeExpr::Value(RuntimeValue::Unit),
                        "item",
                        usize::MAX,
                        crate::task::HostTaskRequestTemplate::new("awbc", "await_many", []),
                    ),
                    resume: None,
                    items: state.items.clone(),
                    next_index: state.next_index as usize,
                    in_flight: Vec::new(),
                    results: state
                        .results
                        .iter()
                        .cloned()
                        .map(|value| value.map(RuntimePayload::from))
                        .collect(),
                }))
            }
            FiberSuspensionReason::Dialogue { .. }
            | FiberSuspensionReason::HostCall { .. }
            | FiberSuspensionReason::BudgetYield => FlowFiberStatus::Running,
        }
    }

    fn line_effect(&self, effect: AwbcEffectPlanId) -> Option<LineEffectRequest> {
        let plan = self.program.effect_plans.get(effect.index())?;
        match plan.kind {
            AwbcEffectKind::Log => Some(LineEffectRequest::Log(RuntimeLog {
                level: "info".to_owned(),
                message: self
                    .effect_static_string(effect, 1)
                    .unwrap_or_else(|| "awbc log".to_owned()),
                fields: Vec::new(),
            })),
            AwbcEffectKind::Call => Some(LineEffectRequest::Call(RuntimeCall {
                callee: self
                    .effect_static_string(effect, 0)
                    .unwrap_or_else(|| "awbc.call".to_owned()),
                args: Vec::new(),
            })),
            AwbcEffectKind::Return => self
                .effect_static_string(effect, 0)
                .map(LineEffectRequest::Return),
            AwbcEffectKind::Goto => self
                .effect_static_string(effect, 0)
                .map(LineEffectRequest::Goto),
            _ => None,
        }
    }

    fn effect_static_string(&self, effect: AwbcEffectPlanId, index: usize) -> Option<String> {
        let constant = self
            .program
            .effect_plans
            .get(effect.index())?
            .static_args
            .get(index)?;
        match self.program.constants.get(constant.index())? {
            crate::awbc::schema::AwbcConstant::String(id) => {
                self.program.strings.get(id.index()).cloned()
            }
            _ => None,
        }
    }

    fn choice_public_id(&self, choice: AwbcChoiceId) -> Option<String> {
        self.program
            .choices
            .get(choice.index())?
            .public_id
            .and_then(|id| self.program.strings.get(id.index()).cloned())
    }

    fn choice_options(&self, choice: AwbcChoiceId) -> Vec<ChoiceRuntimeOption> {
        let Some(choice) = self.program.choices.get(choice.index()) else {
            return Vec::new();
        };
        let start = choice.options.start as usize;
        let end = start.saturating_add(choice.options.len as usize);
        self.program.choice_options[start..end.min(self.program.choice_options.len())]
            .iter()
            .map(|option| ChoiceRuntimeOption {
                id: option
                    .public_id
                    .and_then(|id| self.program.strings.get(id.index()).cloned()),
                label: self
                    .program
                    .strings
                    .get(option.label.index())
                    .cloned()
                    .unwrap_or_else(|| "choice".to_owned()),
                target: None,
                out: None,
                effects: Vec::new(),
            })
            .collect()
    }

    fn first_choice_option(&self, choice: AwbcChoiceId) -> Option<String> {
        self.choice_options(choice)
            .into_iter()
            .next()
            .map(|option| option.id.unwrap_or(option.label))
    }

    fn stream_id(&self, stream: crate::awbc::schema::AwbcStreamPlanId) -> StreamRuntimeId {
        self.program
            .stream_plans
            .get(stream.index())
            .and_then(|plan| self.program.strings.get(plan.public_id.index()))
            .cloned()
            .map_or_else(
                || StreamRuntimeId(format!("awbc.stream.{}", stream.0)),
                StreamRuntimeId,
            )
    }

    fn source_id(&self, source: crate::awbc::schema::AwbcSourcePlanId) -> SourceId {
        self.program
            .source_plans
            .get(source.index())
            .and_then(|plan| self.program.strings.get(plan.public_id.index()))
            .cloned()
            .map_or_else(|| SourceId(format!("awbc.source.{}", source.0)), SourceId)
    }
}

pub fn task_spec_from_awbc_task(
    program: &AwbcProgram,
    plan: AwbcTaskPlanId,
    args: Vec<RuntimeValue>,
) -> Option<TaskSpec> {
    let plan_record = program.task_plans.get(plan.index())?;
    let capability = program.strings.get(plan_record.capability.index())?.clone();
    let operation = program.strings.get(plan_record.operation.index())?.clone();
    let public_id = program
        .strings
        .get(plan_record.public_id.index())
        .cloned()
        .unwrap_or_else(|| format!("awbc.task.{}", plan.0));
    let request_args = args
        .into_iter()
        .map(RuntimePayload::from)
        .collect::<Vec<_>>();
    let request = HostTaskRequest::custom(capability, operation, request_args);
    Some(TaskSpec::new(
        TaskId(public_id.clone()),
        TaskKey(public_id),
        task_class(plan_record.class),
        TaskPriority(plan_record.priority),
        CancelScopeId(
            program
                .strings
                .get(plan_record.cancel_scope.index())
                .cloned()
                .unwrap_or_else(|| "awbc".to_owned()),
        ),
        task_policy(plan_record.policy),
        request,
    ))
}

const fn task_policy(policy: AwbcTaskPolicy) -> TaskPolicy {
    match policy {
        AwbcTaskPolicy::JoinSameKey => TaskPolicy::JoinSameKey,
        AwbcTaskPolicy::AlwaysStart => TaskPolicy::AlwaysStart,
    }
}

const fn task_class(class: AwbcTaskClass) -> crate::task::TaskClass {
    match class {
        AwbcTaskClass::LocalUi => crate::task::TaskClass::LocalUi,
        AwbcTaskClass::Io => crate::task::TaskClass::Io,
        AwbcTaskClass::Cpu => crate::task::TaskClass::Cpu,
        AwbcTaskClass::GpuPrepare => crate::task::TaskClass::GpuPrepare,
        AwbcTaskClass::ShaderCompile => crate::task::TaskClass::ShaderCompile,
        AwbcTaskClass::WasmCall => crate::task::TaskClass::WasmCall,
        AwbcTaskClass::AssetDecode => crate::task::TaskClass::AssetDecode,
        AwbcTaskClass::AudioDecode => crate::task::TaskClass::AudioDecode,
        AwbcTaskClass::AudioRender => crate::task::TaskClass::AudioRender,
        AwbcTaskClass::TtsSynthesis => crate::task::TaskClass::TtsSynthesis,
        AwbcTaskClass::BgmPrecompose => crate::task::TaskClass::BgmPrecompose,
        AwbcTaskClass::Lsp => crate::task::TaskClass::Lsp,
        AwbcTaskClass::Background => crate::task::TaskClass::Background,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepStopReason};

    #[test]
    fn awbc_product_step_empty_program_finishes_without_diagnostics() {
        let mut executor =
            AwbcProductStepExecutor::for_entry(AwbcProgram::default(), AwbcEntryId(0), 64)
                .expect("empty product AWBC executor starts");

        let result = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

        assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
        assert!(result.output.diagnostics.is_empty());
        assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
    }

    #[test]
    fn awbc_product_step_rejects_unreviewed_parity_families() {
        use crate::awbc::schema::{
            AwbcBlock, AwbcRegisterId, AwbcResumePointId, AwbcSafePointKind, AwbcTableRange,
            AwbcTerminator,
        };

        let mut program = AwbcProgram::default();
        program.blocks.push(AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Choice {
                choice: AwbcChoiceId(0),
                dst: AwbcRegisterId(0),
                resume: AwbcResumePointId(0),
            },
            safe_point: AwbcSafePointKind::Choice,
            source_map: None,
        });

        let error = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64)
            .expect_err("choice parity must be blocked before execution");
        assert_eq!(
            error,
            AwbcProductStepBuildError::UnsupportedParity {
                blockers: vec![AwbcProductStepParityBlocker::Choice],
            }
        );
    }

    #[test]
    fn awbc_product_step_parity_inventory_is_deterministic() {
        use crate::awbc::schema::{
            AwbcEffectPlanId, AwbcInstruction, AwbcPureHelperId, AwbcRegisterId,
        };

        let program = AwbcProgram {
            instructions: vec![
                AwbcInstruction::EmitEffect {
                    effect: AwbcEffectPlanId(0),
                    args: Vec::new(),
                },
                AwbcInstruction::CallPureHelper {
                    dst: AwbcRegisterId(0),
                    helper: AwbcPureHelperId(0),
                    args: Vec::new(),
                },
                AwbcInstruction::EmitEffect {
                    effect: AwbcEffectPlanId(0),
                    args: Vec::new(),
                },
            ],
            ..AwbcProgram::default()
        };

        assert_eq!(
            program.product_step_parity_blockers(),
            vec![
                AwbcProductStepParityBlocker::PureHelperCalls,
                AwbcProductStepParityBlocker::Effects,
            ]
        );
    }
}
