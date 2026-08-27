//! Product AWBC runtime-step parity adapter.
//!
//! This module is the sole adapter from canonical compact AWBC execution into
//! the shared `RuntimeStepResult` boundary. It is Sans I/O: every host action is
//! returned as typed data and no structured bytecode fallback is reachable.

mod audio;
mod control;
mod dialogue;
mod execution;
mod lifecycle;
mod line;
mod mapping;
mod root;
mod runtime_id;
mod snapshot;
mod suspension;

use self::dialogue::ProductDialogueStore;
use self::execution::{
    ProductVmHost, has_host_requests, has_visible_output, input_choice_selection, run_function,
    stream_id_for,
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
    runtime_value_matches_type,
};
use crate::awbc::schema::{
    AwbcAwaitObserverResume, AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcEffectPlanId,
    AwbcEntryId, AwbcFunctionId, AwbcHostCallId, AwbcHostCallMode, AwbcLineTaskGroupId,
    AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger, AwbcProgram, AwbcResumePointId,
    AwbcStreamPlanId, AwbcTaskPlanId, AwbcTrapCode, AwbcTypeId,
};
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::awbc::vm::{VmExit, VmObservation, VmStepOptions, step_with_host};
use crate::engine::{
    AwaitState, ChoiceState, FlowExit, FlowFiber, FlowFiberId, FlowFiberOwner, FlowFiberStatus,
    HostCallState,
};
use crate::line_task::{
    AcceptedLineTaskContentEvents, ChildCancelPolicy, ChildJoinPolicy, LineRuntimeError,
    LineTaskExitPolicy, LineTaskLiveState, LineTaskNodeView, LineTaskPlanView, LineTaskTrigger,
    LineTaskWork, LineTaskWorkTag, ScopeExit, cancel_live_line_task_group,
    complete_live_line_task_work, fail_live_line_task_group, finish_live_line_task_group,
    progress_live_line_task_group,
};
use crate::observation::RuntimeObservationState;
use crate::plan::{ChoiceRuntimeOption, FlowEvent};
use crate::pure::{RuntimeCallBackend, VmRuntimePureCallBackend};
use crate::root::RootRuntime;
use crate::step::{
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode,
    RuntimeHostCallRequest, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
    RuntimeStepOutput, RuntimeStepResult, RuntimeStepStats, RuntimeStepStopReason,
};
use crate::stream::{RuntimeStreamEvent, StreamRuntimeState};
use crate::task::{
    AwaitTarget, HostTaskRequestTemplate, NeedId, RuntimeNeedState, TaskEvent, TaskEventKind,
    TaskId, TaskKey, TaskPublicationCursor, TaskSequence, normalize_runtime_need_states,
    normalize_task_events, resolved_runtime_need_state,
};
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeEnv, RuntimeFlowParameterBinding, RuntimeLocalBinding, RuntimePayload, RuntimeValue,
    runtime_sequence_values, runtime_value_label,
};
use arcweft_interaction_model::audio::{AudioCommandEnvelope, AudioDispatchId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

/// Executes one verified stable pure-program binding through its exact AWBC
/// helper row. Callers retain domain ownership of the program identity and
/// arguments; this boundary performs no string lookup or helper fallback.
pub fn evaluate_pure_program_with_backend(
    program: &AwbcProgram,
    pure_program: arcweft_id::runtime_program::RuntimePureProgramId,
    args: &[RuntimeValue],
    backend: &mut impl RuntimeCallBackend,
) -> Result<RuntimeValue, crate::awbc::vm::VmError> {
    let binding = program.pure_program_binding(pure_program).ok_or_else(|| {
        crate::awbc::vm::VmError::Runtime(format!(
            "missing verified AWBC pure program {pure_program}"
        ))
    })?;
    let helper = program
        .pure_helpers
        .get(binding.helper.index())
        .ok_or_else(|| {
            crate::awbc::vm::VmError::Runtime(format!(
                "pure program {pure_program} references missing helper {}",
                binding.helper.0
            ))
        })?;
    if args.len() != binding.input_types.len() {
        return Err(crate::awbc::vm::VmError::FunctionArgumentCount {
            expected: binding.input_types.len(),
            actual: args.len(),
        });
    }
    for (position, (value, expected)) in args.iter().zip(&binding.input_types).enumerate() {
        let ty = program
            .runtime_types
            .iter()
            .position(|ty| ty.semantic_identity() == *expected)
            .and_then(|index| u32::try_from(index).ok())
            .map(AwbcTypeId)
            .ok_or_else(|| {
                crate::awbc::vm::VmError::Runtime(format!(
                    "pure program {pure_program} input {position} references missing semantic type {expected:?}"
                ))
            })?;
        if !runtime_value_matches_type(program, value, ty, 0) {
            return Err(crate::awbc::vm::VmError::Runtime(format!(
                "pure program {pure_program} input {position} violates its exact runtime type"
            )));
        }
    }
    backend.record_awbc_pure_program_call();
    let mut fallback_stats = crate::step::RuntimePureCallStats::default();
    let result = run_function(program, helper.function, args, backend, &mut fallback_stats)?;
    let result_ty = program
        .runtime_types
        .iter()
        .position(|ty| ty.semantic_identity() == binding.result_type)
        .and_then(|index| u32::try_from(index).ok())
        .map(AwbcTypeId)
        .ok_or_else(|| {
            crate::awbc::vm::VmError::Runtime(format!(
                "pure program {pure_program} result references missing semantic type {:?}",
                binding.result_type
            ))
        })?;
    if !runtime_value_matches_type(program, &result, result_ty, 0) {
        return Err(crate::awbc::vm::VmError::Runtime(format!(
            "pure program {pure_program} result violates its exact runtime type"
        )));
    }
    Ok(result)
}

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
    #[error("failed to derive the accepted product AWBC artifact identity: {message}")]
    ArtifactIdentity { message: String },
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
    #[error(transparent)]
    Line(#[from] crate::line_task::LineRuntimeError),
    #[error(transparent)]
    LineTaskCompletion(#[from] crate::line_task::LineTaskCompletionError),
    #[error("product AWBC dialogue content identity overflowed")]
    DialogueContentIdentityOverflow,
    #[error("product AWBC dialogue occurrence identity overflowed")]
    DialogueOccurrenceOverflow,
    #[error("product AWBC child generation identity overflowed")]
    ChildGenerationOverflow,
    #[error("product AWBC dialogue line cursor overflowed")]
    DialogueLineCursorOverflow,
    #[error(
        "product AWBC line-task child completed for content {actual:?}, active dialogue is {expected:?}"
    )]
    StaleLineTaskChildContent {
        expected: AwbcContentUnitId,
        actual: AwbcContentUnitId,
    },
    #[error(transparent)]
    RuntimeIdentity(#[from] crate::runtime_id::RuntimeIdExhausted),
    #[error(transparent)]
    Fiber(#[from] crate::awbc::fiber::FiberStateError),
}

impl From<crate::presentation::RuntimeCommandQueueError> for ProductStepError {
    fn from(error: crate::presentation::RuntimeCommandQueueError) -> Self {
        Self::Line(crate::line_task::LineRuntimeError::from(error))
    }
}

impl ProductStepError {
    const fn category(&self) -> RuntimeDiagnosticCategory {
        match self {
            Self::Input(_) => RuntimeDiagnosticCategory::Input,
            Self::Type(_) => RuntimeDiagnosticCategory::Type,
            Self::Host(_) => RuntimeDiagnosticCategory::Host,
            Self::Internal(_)
            | Self::Line(_)
            | Self::LineTaskCompletion(_)
            | Self::DialogueContentIdentityOverflow
            | Self::DialogueOccurrenceOverflow
            | Self::ChildGenerationOverflow
            | Self::DialogueLineCursorOverflow
            | Self::StaleLineTaskChildContent { .. }
            | Self::RuntimeIdentity(_)
            | Self::Fiber(_) => RuntimeDiagnosticCategory::Internal,
        }
    }

    const fn trap_code(&self) -> AwbcTrapCode {
        match self {
            Self::Type(_) => AwbcTrapCode::TypeMismatch,
            Self::Host(_) => AwbcTrapCode::HostAbiMismatch,
            Self::Input(_)
            | Self::Internal(_)
            | Self::Line(_)
            | Self::LineTaskCompletion(_)
            | Self::DialogueContentIdentityOverflow
            | Self::DialogueOccurrenceOverflow
            | Self::ChildGenerationOverflow
            | Self::DialogueLineCursorOverflow
            | Self::StaleLineTaskChildContent { .. }
            | Self::RuntimeIdentity(_)
            | Self::Fiber(_) => AwbcTrapCode::InternalInvariant,
        }
    }
}

fn partition_drop_observation(
    observations: Vec<VmObservation>,
) -> Result<
    (Option<crate::effect::RuntimeDropPolicy>, Vec<VmObservation>),
    crate::line_task::LineRuntimeError,
> {
    let mut drop_policy = None;
    let mut remaining = Vec::with_capacity(observations.len());
    for observation in observations {
        match observation {
            VmObservation::Drop { policy } => {
                if drop_policy.replace(policy).is_some() {
                    return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation);
                }
            }
            observation => remaining.push(observation),
        }
    }
    Ok((drop_policy, remaining))
}

#[derive(Clone, Debug, PartialEq)]
struct ActiveDialogue {
    activation: crate::runtime_id::DialogueActivationId,
    content: AwbcContentUnitId,
    line: crate::plan::RuntimeLineId,
    captures: Box<[RuntimeValue]>,
    values: Box<[crate::plan::RuntimeDialogueValueBinding]>,
    voice: crate::presentation::RuntimeDialogueVoiceState,
    result: crate::awbc::schema::AwbcDialogueResultTarget,
    phase: ProductDialoguePhase,
    elapsed_nanos: u64,
    pending_content_events: Vec<crate::step::RuntimeDialogueContentEventKind>,
    pending_advance: bool,
    pending_line_outcomes: Vec<crate::presentation::RuntimeLineHostOutcome>,
}

#[derive(Clone, Debug, PartialEq)]
enum ProductDialoguePhase {
    Activating {
        fiber: FiberState,
        pending: Option<ProductPendingLineOperation>,
    },
    Reducing {
        line_task: LineTaskLiveState,
    },
    Publishing {
        line_task: LineTaskLiveState,
    },
    Closing(ProductDialogueClosing),
}

#[derive(Clone, Debug, PartialEq)]
struct ProductDialogueClosing {
    failure: FiberTrap,
    state: ProductDialogueClosingState,
}

#[derive(Clone, Debug, PartialEq)]
enum ProductDialogueClosingState {
    Activation {
        fiber: FiberState,
        pending: Option<ProductPendingLineOperation>,
    },
    LineTask {
        line_task: LineTaskLiveState,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum ProductPendingLineOperation {
    AcquireActor {
        cursor: FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        value: RuntimeValue,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    ActorLook {
        cursor: FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        value: RuntimeValue,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    StartVoice {
        cursor: FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        site: crate::awbc::schema::AwbcLineHandleSiteId,
    },
}

impl ProductPendingLineOperation {
    fn command(&self) -> &crate::presentation::RuntimeLineCommandId {
        match self {
            Self::AcquireActor { command, .. }
            | Self::ActorLook { command, .. }
            | Self::StartVoice { command, .. } => command,
        }
    }
}

impl ActiveDialogue {
    fn line_task(&self) -> Option<&LineTaskLiveState> {
        match &self.phase {
            ProductDialoguePhase::Reducing { line_task }
            | ProductDialoguePhase::Publishing { line_task } => Some(line_task),
            ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::LineTask { line_task },
                ..
            }) => Some(line_task),
            ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::Activation { .. },
                ..
            }) => None,
            ProductDialoguePhase::Activating { .. } => None,
        }
    }

    fn line_task_mut(&mut self) -> Option<&mut LineTaskLiveState> {
        match &mut self.phase {
            ProductDialoguePhase::Reducing { line_task }
            | ProductDialoguePhase::Publishing { line_task } => Some(line_task),
            ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::LineTask { line_task },
                ..
            }) => Some(line_task),
            ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::Activation { .. },
                ..
            }) => None,
            ProductDialoguePhase::Activating { .. } => None,
        }
    }

    fn is_ingress_ready(&self) -> bool {
        matches!(
            &self.phase,
            ProductDialoguePhase::Reducing { line_task }
                if !line_task.is_closing() && !line_task.is_closed()
        )
    }
}

impl AwbcProductStepExecutor {
    fn dialogue_group(&self, content: AwbcContentUnitId) -> Option<AwbcLineTaskGroupId> {
        self.program
            .content_units
            .get(content.index())
            .and_then(|content| content.line_task_group)
    }

    fn runtime_dialogue_content_id(
        content: AwbcContentUnitId,
    ) -> Result<crate::runtime_id::RuntimeDialogueContentPlanId, ProductStepError> {
        let ordinal = content
            .0
            .checked_add(1)
            .and_then(std::num::NonZeroU32::new)
            .ok_or(ProductStepError::DialogueContentIdentityOverflow)?;
        Ok(crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(ordinal))
    }

    fn prepare_dialogue_activation(
        &self,
        content: AwbcContentUnitId,
    ) -> Result<
        (
            crate::runtime_id::DialogueActivationId,
            (
                crate::runtime_id::RuntimePersistentFiberId,
                crate::runtime_id::RuntimeDialogueContentPlanId,
            ),
            u64,
        ),
        ProductStepError,
    > {
        let content = Self::runtime_dialogue_content_id(content)?;
        let owner = self.facade_fiber.persistent_id;
        let key = (owner, content);
        let occurrence = self.dialogue_occurrences.get(&key).copied().unwrap_or(0);
        let next = occurrence
            .checked_add(1)
            .ok_or(ProductStepError::DialogueOccurrenceOverflow)?;
        Ok((
            crate::runtime_id::DialogueActivationId::new(
                self.artifact_fingerprint,
                owner,
                content,
                occurrence,
            ),
            key,
            next,
        ))
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

/// Fallible realization of one reducer command batch. Child fibers and their
/// identity cursors are prepared off to the side; the dialogue registry and
/// this executor substate are committed only after every command has been
/// validated and materialized.
struct ProductLineTaskExecutionBatch {
    child_fibers: VecDeque<ProductChildFiber>,
    next_generation: u64,
    next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    observations: Vec<VmObservation>,
    pure_stats: Option<crate::step::RuntimePureCallStats>,
}

impl ProductLineTaskExecutionBatch {
    fn has_joined_dialogue_work(
        &self,
        activation: &crate::runtime_id::DialogueActivationId,
    ) -> bool {
        self.child_fibers.iter().any(|child| {
            matches!(
                &child.owner,
                ProductChildFiberOwner::LineTask { tag, policy, .. }
                    if tag.activation_id() == activation
                        && policy.join == ChildJoinPolicy::Join
            )
        })
    }
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

    fn function_for(&self, tag: &LineTaskWorkTag) -> Option<AwbcFunctionId> {
        match tag.work() {
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
                    AwbcLineTaskTrigger::ContentEffect(site) => {
                        LineTaskTrigger::ContentEffect(*site)
                    }
                    AwbcLineTaskTrigger::Scheduled(site) => LineTaskTrigger::Scheduled(
                        crate::runtime_id::RuntimeLineHandleSiteId::from_zero_based(site.0),
                    ),
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

    fn scheduled_child(
        &self,
        site: crate::runtime_id::RuntimeLineHandleSiteId,
    ) -> Option<crate::runtime_id::RuntimeLineTaskNodeId> {
        self.group
            .handle_sites
            .get(site.index())?
            .scheduled_child
            .and_then(|child| self.global_node_to_local(child))
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
    Shared(Box<FlowFiberStatus>),
    WaitingMany(FiberAwaitManyState),
}

/// Stateful canonical AWBC executor exposed through `RuntimeStepResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductStepExecutor {
    pub(super) program: AwbcProgram,
    artifact_fingerprint: crate::effect::RuntimeArtifactFingerprint,
    fiber: FiberState,
    facade_fiber: FlowFiber,
    entry_bound: bool,
    dialogues: ProductDialogueStore,
    active_choice: Option<ActiveChoice>,
    pending_host_call: Option<PendingHostCall>,
    started_tasks: BTreeSet<TaskId>,
    task_publications: BTreeMap<TaskId, TaskPublicationCursor>,
    need_publications: BTreeMap<NeedId, TaskPublicationCursor>,
    queued_task_events: VecDeque<TaskEvent>,
    emitted_content: BTreeSet<AwbcContentUnitId>,
    stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    child_fibers: VecDeque<ProductChildFiber>,
    dialogue_occurrences: BTreeMap<
        (
            crate::runtime_id::RuntimePersistentFiberId,
            crate::runtime_id::RuntimeDialogueContentPlanId,
        ),
        u64,
    >,
    next_generation: u64,
    next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    next_host_call_sequence: u64,
    next_audio_sequence: u64,
    compact_pure_stats: crate::step::RuntimePureCallStats,
    root: Option<RootRuntime>,
}

impl AwbcProductStepExecutor {
    fn artifact_fingerprint(
        program: &AwbcProgram,
    ) -> Result<crate::effect::RuntimeArtifactFingerprint, AwbcProductStepBuildError> {
        let encoded = program.encode_canonical().map_err(|error| {
            AwbcProductStepBuildError::ArtifactIdentity {
                message: error.to_string(),
            }
        })?;
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes(
            *blake3::hash(&encoded).as_bytes(),
        )
        .map_err(|error| AwbcProductStepBuildError::ArtifactIdentity {
            message: error.to_string(),
        })
    }

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
        candidate.artifact_fingerprint = Self::artifact_fingerprint(&program)?;
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
                instance: crate::runtime_id::RuntimeFiberInstanceId::from_allocated(
                    std::num::NonZeroU64::MIN,
                ),
                next_frame_instance: crate::runtime_id::RuntimeIdCursor::initial(),
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
        if root_startup.is_none() && !fiber.frames.is_empty() {
            fiber
                .bind_flow_parameter_coordinates(&program, &[])
                .map_err(|error| AwbcProductStepBuildError::FiberState {
                    message: error.to_string(),
                })?;
        }
        let artifact_fingerprint = Self::artifact_fingerprint(&program)?;
        let mut executor = Self::for_fiber(program, fiber, artifact_fingerprint);
        executor.entry_bound = true;
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
        Self::for_function_invocation(program, entry, function, [], budget_quantum)
    }

    /// Creates a route-selected product executor and consumes the complete
    /// checked Flow parameter invocation before the first instruction.
    pub fn for_function_invocation(
        program: AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        bindings: impl IntoIterator<Item = RuntimeFlowParameterBinding>,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProductStepBuildError> {
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        let mut fiber = FiberState::for_entry_target_function(
            &program,
            entry,
            function,
            0,
            budget_quantum.max(1),
        )
        .map_err(|error| AwbcProductStepBuildError::FiberState {
            message: error.to_string(),
        })?;
        let bindings = bindings.into_iter().collect::<Vec<_>>();
        fiber
            .bind_flow_parameter_coordinates(&program, &bindings)
            .map_err(|error| AwbcProductStepBuildError::FiberState {
                message: error.to_string(),
            })?;
        let artifact_fingerprint = Self::artifact_fingerprint(&program)?;
        let mut executor = Self::for_fiber(program, fiber, artifact_fingerprint);
        executor.entry_bound = true;
        Ok(executor)
    }

    fn for_fiber(
        program: AwbcProgram,
        fiber: FiberState,
        artifact_fingerprint: crate::effect::RuntimeArtifactFingerprint,
    ) -> Self {
        let mut next_fiber_instance = crate::runtime_id::RuntimeIdCursor::initial();
        let main_fiber_instance = next_fiber_instance
            .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
            .expect("initial Product fiber identity is available");
        debug_assert_eq!(fiber.instance.get(), main_fiber_instance);
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
            persistent_id: crate::runtime_id::RuntimePersistentFiberId::from_allocated(1),
            execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                std::num::NonZeroU64::MIN,
            ),
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
            artifact_fingerprint,
            fiber,
            facade_fiber,
            entry_bound: false,
            dialogues: ProductDialogueStore::default(),
            active_choice: None,
            pending_host_call: None,
            started_tasks: BTreeSet::new(),
            task_publications: BTreeMap::new(),
            need_publications: BTreeMap::new(),
            queued_task_events: VecDeque::new(),
            emitted_content: BTreeSet::new(),
            stream_sequences: BTreeMap::new(),
            child_fibers: VecDeque::new(),
            dialogue_occurrences: BTreeMap::new(),
            next_generation: 1,
            next_fiber_instance,
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

    fn latch_dialogue_step_input(
        &mut self,
        input: &mut RuntimeStepInput,
    ) -> Result<Vec<crate::line_task::LineRuntimeError>, ProductStepError> {
        let content_events = std::mem::take(&mut input.dialogue_content_events);
        let advances = std::mem::take(&mut input.dialogue_advances);
        let line_outcomes = std::mem::take(&mut input.line_outcomes);
        self.dialogues
            .latch_step_input(input.dt, &content_events, &advances, &line_outcomes)
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
        mut input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        let pure_before = pure_backend.stats();
        let local_pure_before = self.compact_pure_stats;
        let mut output = RuntimeStepOutput::default();
        let executed_ops = 0_usize;
        let pending_ops_before = self.pending_ops_len();
        let root_events_in = input.root_events.len();
        let deferred_root_events = std::mem::take(&mut input.deferred_root_events);
        output
            .requests
            .root_events_next_step
            .extend(deferred_root_events);

        let ingress_diagnostics = match self.latch_dialogue_step_input(&mut input) {
            Ok(diagnostics) => diagnostics,
            Err(error) => {
                self.fail_with_error(error, &mut output);
                self.sync_facade();
                let stop_reason = self.stop_reason(options, executed_ops, &output);
                let diagnostics = output.diagnostics.len();
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
                        diagnostics,
                        ..RuntimeStepStats::default()
                    },
                );
            }
        };
        for diagnostic in ingress_diagnostics {
            self.record_error(ProductStepError::Line(diagnostic), &mut output);
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
        let need_states = normalize_runtime_need_states(std::mem::take(&mut input.need_states));
        let task_events = normalize_task_events(std::mem::take(&mut input.task_events));
        let need_states_in = need_states.len();
        let task_events_in = task_events.len();
        Self::append_task_event_diagnostics(&mut output, &task_events);
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

        let executed_ops = self.run_main_work(
            &input,
            &need_states,
            &task_events,
            &mut output,
            options,
            pure_backend,
        );

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

    fn append_task_event_diagnostics(output: &mut RuntimeStepOutput, events: &[TaskEvent]) {
        output.diagnostics.extend(events.iter().map(|event| {
            RuntimeDiagnostic::new(format!(
                "task {} sequence {} delivered",
                event.task_id.0, event.sequence.0
            ))
        }));
    }

    fn run_main_work(
        &mut self,
        input: &RuntimeStepInput,
        need_states: &[RuntimeNeedState],
        task_events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> usize {
        let mut executed_ops = 0_usize;
        while executed_ops < options.budget.max_ops && self.has_attemptable_work() {
            if self.fiber.status == FiberStatus::Suspended {
                let progressed = self.resume_main_suspension(
                    input,
                    need_states,
                    task_events,
                    output,
                    pure_backend,
                );
                executed_ops = executed_ops.saturating_add(usize::from(progressed));
                if !progressed || self.should_return_to_host(options.mode, output, executed_ops) {
                    break;
                }
                continue;
            }
            let line_effects_before = output.effects.line.len();
            if self.fiber.status == FiberStatus::Running {
                executed_ops = executed_ops.saturating_add(self.step_main_vm(
                    need_states,
                    output,
                    pure_backend,
                ));
            } else if !self.step_next_child(output, pure_backend) {
                break;
            } else {
                executed_ops = executed_ops.saturating_add(1);
            }
            self.apply_control_effects(output, line_effects_before);
            if self.should_return_to_host(options.mode, output, executed_ops) {
                break;
            }
        }
        executed_ops
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
        let before = self.fiber.clone();
        let mut candidate = before.clone();
        let mut candidate_stats = self.compact_pure_stats.clone();
        let mut host = ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut candidate_stats,
        };
        match step_with_host(
            &self.program,
            &mut candidate,
            VmStepOptions {
                max_instructions: 1,
            },
            &mut host,
        ) {
            Ok(vm_output) => {
                let (drop_policy, observations) =
                    match partition_drop_observation(vm_output.observations) {
                        Ok(parts) => parts,
                        Err(error) => {
                            self.fail_with_error(error.into(), output);
                            return usize::try_from(vm_output.executed).unwrap_or(usize::MAX);
                        }
                    };
                let before_owners =
                    match line::product_fiber_handle_owners(self.facade_fiber.execution, &before) {
                        Ok(owners) => owners,
                        Err(error) => {
                            self.fail_with_error(error, output);
                            return usize::try_from(vm_output.executed).unwrap_or(usize::MAX);
                        }
                    };
                let after_owners = match line::product_fiber_handle_owners(
                    self.facade_fiber.execution,
                    &candidate,
                ) {
                    Ok(owners) => owners,
                    Err(error) => {
                        self.fail_with_error(error, output);
                        return usize::try_from(vm_output.executed).unwrap_or(usize::MAX);
                    }
                };
                let drop_receipt = match self.dialogues.reconcile_parent_fiber(
                    self.facade_fiber.execution,
                    &before_owners,
                    &after_owners,
                    drop_policy,
                ) {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        self.fail_with_error(error.into(), output);
                        return usize::try_from(vm_output.executed).unwrap_or(usize::MAX);
                    }
                };
                self.fiber = candidate;
                self.compact_pure_stats = candidate_stats;
                output
                    .requests
                    .line_commands
                    .extend(drop_receipt.into_commands());
                self.consume_observations(observations, output);
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
        let Some(front) = self.child_fibers.front() else {
            return false;
        };
        if front.fiber.status != FiberStatus::Running {
            if let Some(child) = self.child_fibers.pop_front() {
                self.child_fibers.push_back(child);
            }
            return false;
        }
        let mut remaining = self.child_fibers.clone();
        let Some(mut child) = remaining.pop_front() else {
            return false;
        };
        let owner = child.owner.clone();
        let before_handles = match &owner {
            ProductChildFiberOwner::LineTask { .. } => {
                match line::product_fiber_handle_owners(self.facade_fiber.execution, &child.fiber) {
                    Ok(handles) => Some(handles.into_keys().collect::<BTreeSet<_>>()),
                    Err(error) => {
                        let ProductChildFiberOwner::LineTask { tag, .. } = &owner else {
                            unreachable!("line-task handle scan has a line-task owner")
                        };
                        return self.begin_product_line_task_child_failure(tag, error, output);
                    }
                }
            }
            ProductChildFiberOwner::Independent => None,
        };
        child.fiber.replenish_budget();
        let mut candidate_stats = self.compact_pure_stats.clone();
        let mut host = ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut candidate_stats,
        };
        let vm_output = match step_with_host(
            &self.program,
            &mut child.fiber,
            VmStepOptions {
                max_instructions: 1,
            },
            &mut host,
        ) {
            Ok(vm_output) => vm_output,
            Err(error) => {
                if let ProductChildFiberOwner::LineTask { tag, .. } = &owner {
                    return self.begin_product_line_task_child_failure(
                        tag,
                        ProductStepError::Internal(error.to_string()),
                        output,
                    );
                }
                self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                return true;
            }
        };
        let (drop_policy, observations) = match partition_drop_observation(vm_output.observations) {
            Ok(parts) => parts,
            Err(error) => {
                if let ProductChildFiberOwner::LineTask { tag, .. } = &owner {
                    return self.begin_product_line_task_child_failure(tag, error.into(), output);
                }
                self.fail_with_error(error.into(), output);
                return true;
            }
        };
        if matches!(&owner, ProductChildFiberOwner::Independent) {
            if matches!(
                child.fiber.status,
                FiberStatus::Running | FiberStatus::Suspended
            ) {
                remaining.push_back(child);
            }
            self.child_fibers = remaining;
            self.compact_pure_stats = candidate_stats;
            self.consume_observations(observations, output);
            return true;
        }
        let ProductChildFiberOwner::LineTask {
            content,
            tag,
            policy,
            phase,
            ..
        } = owner
        else {
            self.fail_with_error(
                crate::line_task::LineRuntimeError::InvalidActivationOperation.into(),
                output,
            );
            return true;
        };
        if child.fiber.status == FiberStatus::Suspended {
            return self.begin_product_line_task_child_failure(
                &tag,
                ProductStepError::Internal(
                    "AWBC line-task action suspended without an owned resume protocol".to_owned(),
                ),
                output,
            );
        }
        let mut transaction = match self.dialogues.begin_transaction(tag.activation_id()) {
            Ok(transaction) => transaction,
            Err(error) => {
                self.fail_with_error(error.into(), output);
                return true;
            }
        };
        let failure_transaction = transaction.clone();
        let after_handles =
            match line::product_fiber_handle_owners(self.facade_fiber.execution, &child.fiber) {
                Ok(handles) => handles.into_keys().collect::<BTreeSet<_>>(),
                Err(error) => {
                    return self.begin_product_dialogue_failure(failure_transaction, error, output);
                }
            };
        let Some(before_handles) = before_handles.as_ref() else {
            return self.begin_product_dialogue_failure(
                failure_transaction,
                LineRuntimeError::InvalidActivationOperation.into(),
                output,
            );
        };
        if let Err(error) = transaction.line_mut().reconcile_child_scope_step(
            &tag,
            before_handles,
            &after_handles,
            drop_policy,
        ) {
            return self.begin_product_dialogue_failure(failure_transaction, error.into(), output);
        }
        if child.fiber.status == FiberStatus::Running {
            remaining.push_back(child);
            let receipt = match self.dialogues.commit(transaction) {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.fail_with_error(error.into(), output);
                    return true;
                }
            };
            self.child_fibers = remaining;
            self.compact_pure_stats = candidate_stats;
            let commands = receipt.into_line().into_commands();
            output.requests.line_commands.extend(commands);
            self.consume_observations(observations, output);
            return true;
        }
        let failed = child.fiber.status == FiberStatus::Trapped;
        let cancelled = child.fiber.status == FiberStatus::Cancelled
            || phase == ProductLineTaskFiberPhase::Closing;
        let batch = ProductLineTaskExecutionBatch {
            child_fibers: remaining,
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            observations,
            pure_stats: Some(candidate_stats),
        };
        let batch = match self.prepare_owned_line_task_completion(
            &mut transaction,
            content,
            tag,
            &mut child.fiber,
            failed,
            cancelled,
            policy.join == ChildJoinPolicy::Join,
            batch,
        ) {
            Ok(batch) => batch,
            Err(error) => {
                return self.begin_product_dialogue_failure(failure_transaction, error, output);
            }
        };
        if let Some(FiberTerminalValue::Trapped(trap)) = child.fiber.terminal.as_ref() {
            let trap = trap.clone();
            self.record_trap(&trap, output);
            let (transaction, batch) =
                match self.prepare_product_dialogue_failure(transaction, trap, batch) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        self.fail_with_error(error, output);
                        return true;
                    }
                };
            return self.commit_product_dialogue_failure_close(transaction, batch, output);
        }
        let receipt = match self.dialogues.commit(transaction) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.fail_with_error(error.into(), output);
                return true;
            }
        };
        self.commit_line_task_commands(batch, output);
        let commands = receipt.into_line().into_commands();
        output.requests.line_commands.extend(commands);
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
                result,
            } => self.present_dialogue(content, values, line_task_captures, result, output),
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
                result,
            } => self.resume_dialogue(
                content,
                values,
                line_task_captures,
                result,
                resume,
                output,
                pure_backend,
            ),
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
        result: crate::awbc::schema::AwbcDialogueResultTarget,
        output: &mut RuntimeStepOutput,
    ) {
        if self
            .dialogues
            .active_frame()
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
        let (activation, occurrence_key, next_occurrence) =
            match self.prepare_dialogue_activation(content) {
                Ok(prepared) => prepared,
                Err(error) => {
                    self.record_error(error, output);
                    return;
                }
            };
        let Some(group_id) = self.dialogue_group(content) else {
            self.record_error(
                ProductStepError::Line(crate::line_task::LineRuntimeError::MissingTaskGroup),
                output,
            );
            return;
        };
        let Some(group) = self.program.line_task_groups.get(group_id.index()) else {
            self.record_error(
                ProductStepError::Line(crate::line_task::LineRuntimeError::UnknownTaskGroup),
                output,
            );
            return;
        };
        if group.result_type != result.ty {
            self.record_error(
                ProductStepError::Line(
                    crate::line_task::LineRuntimeError::DialogueResultTypeMismatch,
                ),
                output,
            );
            return;
        }
        if captures
            .iter()
            .any(|capture| !capture.ownership().permits_copy())
        {
            self.record_error(
                crate::line_task::LineRuntimeError::AffineGroupCapture.into(),
                output,
            );
            return;
        }
        let Some(next_generation) = self.next_generation.checked_add(1) else {
            self.record_error(ProductStepError::ChildGenerationOverflow, output);
            return;
        };
        let Some(next_line_cursor) = self.fiber.line_cursor.checked_add(1) else {
            self.record_error(ProductStepError::DialogueLineCursorOverflow, output);
            return;
        };
        let mut next_fiber_instance = self.next_fiber_instance;
        let activation_fiber_instance = match next_fiber_instance
            .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
        {
            Ok(instance) => crate::runtime_id::RuntimeFiberInstanceId::from_allocated(instance),
            Err(error) => {
                self.record_error(error.into(), output);
                return;
            }
        };
        let mut activation_fiber = match FiberState::for_function_with_instance(
            &self.program,
            self.fiber.entry,
            group.activation,
            activation_fiber_instance,
            self.next_generation,
            self.fiber.budget.quantum.max(1),
        ) {
            Ok(fiber) => fiber,
            Err(error) => {
                self.record_error(ProductStepError::Internal(error.to_string()), output);
                return;
            }
        };
        if let Err(error) = activation_fiber.bind_function_argument_values(&self.program, &captures)
        {
            self.record_error(ProductStepError::Type(error.to_string()), output);
            return;
        }
        let active = ActiveDialogue {
            activation,
            content,
            line: line_id,
            captures,
            values,
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result,
            phase: ProductDialoguePhase::Activating {
                fiber: activation_fiber,
                pending: None,
            },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
        };
        let mut dialogues = self.dialogues.clone();
        if let Err(error) = dialogues.begin(active) {
            self.fail_with_error(error.into(), output);
            return;
        }
        self.dialogues = dialogues;
        self.dialogue_occurrences
            .insert(occurrence_key, next_occurrence);
        self.next_generation = next_generation;
        self.next_fiber_instance = next_fiber_instance;
        self.fiber.line_cursor = next_line_cursor;
    }

    fn resume_dialogue(
        &mut self,
        content: AwbcContentUnitId,
        values: Box<[crate::plan::RuntimeDialogueValueBinding]>,
        captures: Box<[RuntimeValue]>,
        result: crate::awbc::schema::AwbcDialogueResultTarget,
        resume: AwbcResumePointId,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        if self.dialogues.active_frame().is_none() {
            self.present_dialogue(content, values, captures.clone(), result, output);
        }
        let Ok(mut transaction) = self.dialogues.begin_active_transaction() else {
            return false;
        };
        if matches!(transaction.frame().phase, ProductDialoguePhase::Closing(_)) {
            return self.resume_product_dialogue_failure_close(transaction, output);
        }
        if matches!(
            transaction.frame().phase,
            ProductDialoguePhase::Activating { .. }
        ) {
            let progress = match self.step_dialogue_activation(&mut transaction, pure_backend) {
                Ok(progress) => progress,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
            let candidate_pure_stats = progress.pure_stats.clone();
            let command_batch =
                match self.prepare_line_task_commands(&mut transaction, progress.reducer) {
                    Ok(batch) => batch,
                    Err(error) => {
                        return self.begin_product_dialogue_failure(transaction, error, output);
                    }
                };
            let receipt = match self.dialogues.commit(transaction) {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.fail_with_error(error.into(), output);
                    return true;
                }
            };
            if let Some(stats) = candidate_pure_stats {
                self.compact_pure_stats = stats;
            }
            self.commit_line_task_commands(command_batch, output);
            let commands = receipt.into_line().into_commands();
            output.requests.line_commands.extend(commands);
            if let Some(event) = progress.presented {
                self.emitted_content.insert(content);
                output.flow_events.push(event);
            }
            return progress.progressed;
        }
        let outcomes = std::mem::take(&mut transaction.frame_mut().pending_line_outcomes);
        match transaction.line_mut().accept_runtime_outcomes(&outcomes) {
            Ok(diagnostics) if diagnostics.is_empty() => {}
            Ok(mut diagnostics) => {
                let error = diagnostics.remove(0);
                return self.begin_product_dialogue_failure(transaction, error.into(), output);
            }
            Err(error) => {
                return self.begin_product_dialogue_failure(transaction, error.into(), output);
            }
        }
        let elapsed = LogicalDuration::from_nanos(transaction.frame().elapsed_nanos);
        let due = match transaction.line_mut().arm_due_schedules(elapsed) {
            Ok(due) => due,
            Err(error) => {
                return self.begin_product_dialogue_failure(transaction, error.into(), output);
            }
        };
        let active = transaction.frame_mut();
        let content_events = std::mem::take(&mut active.pending_content_events);
        let advance = std::mem::take(&mut active.pending_advance);
        let Some(content_unit) = self
            .program
            .content_units
            .get(active.content.index())
            .cloned()
        else {
            return self.begin_product_dialogue_failure(
                transaction,
                ProductStepError::Internal(
                    "active dialogue references an absent AWBC content unit".to_owned(),
                ),
                output,
            );
        };
        let accepted_content = match active.line_task_mut() {
            Some(line_task) => {
                for token in due {
                    if let Err(error) = line_task.mark_scheduled_ready(token) {
                        return self.begin_product_dialogue_failure(
                            transaction,
                            error.into(),
                            output,
                        );
                    }
                }
                line_task.accept_content_event_kinds(&content_events, |event| match event {
                    crate::step::RuntimeDialogueContentEventKind::Mark(mark) => content_unit
                        .marks
                        .get(mark.index())
                        .is_some_and(|row| row.id == mark),
                    crate::step::RuntimeDialogueContentEventKind::Effect(effect) => {
                        effect.get().get() <= content_unit.effect_site_count
                    }
                })
            }
            None if content_events.is_empty() => Ok(AcceptedLineTaskContentEvents::default()),
            None => Err(
                crate::line_task::LineRuntimeError::ContentEventOutsideLiveLineTask {
                    event: content_events[0],
                },
            ),
        };
        let accepted_content = match accepted_content {
            Ok(events) => events,
            Err(error) => {
                return self.begin_product_dialogue_failure(
                    transaction,
                    ProductStepError::Input(error.to_string()),
                    output,
                );
            }
        };
        let mut reducer_activation = match self.progress_line_task(active, &accepted_content) {
            Ok(activation) => activation,
            Err(error) => {
                return self.begin_product_dialogue_failure(transaction, error, output);
            }
        };
        let (cancel_trigger, cancel_activation) =
            match self.cancel_line_task(active, accepted_content.marks()) {
                Ok(result) => result,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
        reducer_activation.append(cancel_activation);
        if advance {
            let finish_activation = match self.finish_line_task(active) {
                Ok(activation) => activation,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
            reducer_activation.append(finish_activation);
        }
        let command_batch =
            match self.prepare_line_task_commands(&mut transaction, reducer_activation) {
                Ok(batch) => batch,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
        if transaction
            .frame()
            .line_task()
            .is_some_and(LineTaskLiveState::is_closed)
        {
            let publication = match self.prepare_dialogue_publication(&mut transaction, resume) {
                Ok(publication) => publication,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
            return match publication {
                line::ProductPublicationProgress::Pending => {
                    let receipt = match self.dialogues.commit(transaction) {
                        Ok(receipt) => receipt,
                        Err(error) => {
                            self.fail_with_error(error.into(), output);
                            return false;
                        }
                    };
                    let commands = receipt.into_line().into_commands();
                    self.commit_line_task_commands(command_batch, output);
                    output.requests.line_commands.extend(commands);
                    if let Some(trigger) = cancel_trigger {
                        output.flow_events.push(FlowEvent::LineCancelled {
                            trigger: self.dialogue_mark_label(content, trigger),
                        });
                    }
                    false
                }
                line::ProductPublicationProgress::Ready(parent) => {
                    let receipt = match self.dialogues.commit_published(transaction) {
                        Ok(receipt) => receipt,
                        Err(error) => {
                            self.fail_with_error(error.into(), output);
                            return false;
                        }
                    };
                    let commands = receipt.into_line().into_commands();
                    self.commit_line_task_commands(command_batch, output);
                    output.requests.line_commands.extend(commands);
                    if let Some(trigger) = cancel_trigger {
                        output.flow_events.push(FlowEvent::LineCancelled {
                            trigger: self.dialogue_mark_label(content, trigger),
                        });
                    }
                    self.fiber = parent;
                    true
                }
            };
        }
        let receipt = match self.dialogues.commit(transaction) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.fail_with_error(error.into(), output);
                return false;
            }
        };
        self.commit_line_task_commands(command_batch, output);
        let commands = receipt.into_line().into_commands();
        output.requests.line_commands.extend(commands);
        if let Some(trigger) = cancel_trigger {
            output.flow_events.push(FlowEvent::LineCancelled {
                trigger: self.dialogue_mark_label(content, trigger),
            });
        }
        false
    }

    fn begin_product_dialogue_failure(
        &mut self,
        transaction: dialogue::ProductDialogueTransaction,
        error: ProductStepError,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let message = error.to_string();
        let trap = FiberTrap {
            code: error.trap_code(),
            message: Some(message),
            source_map: None,
        };
        self.record_error(error, output);
        let batch = ProductLineTaskExecutionBatch {
            child_fibers: self.child_fibers.clone(),
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            observations: Vec::new(),
            pure_stats: None,
        };
        let (transaction, batch) =
            match self.prepare_product_dialogue_failure(transaction, trap, batch) {
                Ok(prepared) => prepared,
                Err(cleanup) => {
                    self.fail_with_error(cleanup, output);
                    return true;
                }
            };
        self.commit_product_dialogue_failure_close(transaction, batch, output)
    }

    fn begin_product_line_task_child_failure(
        &mut self,
        tag: &LineTaskWorkTag,
        error: ProductStepError,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let transaction = match self.dialogues.begin_transaction(tag.activation_id()) {
            Ok(transaction) => transaction,
            Err(registry) => {
                self.fail_with_error(registry.into(), output);
                return true;
            }
        };
        self.begin_product_dialogue_failure(transaction, error, output)
    }

    fn prepare_product_dialogue_failure(
        &self,
        mut transaction: dialogue::ProductDialogueTransaction,
        trap: FiberTrap,
        batch: ProductLineTaskExecutionBatch,
    ) -> Result<
        (
            dialogue::ProductDialogueTransaction,
            ProductLineTaskExecutionBatch,
        ),
        ProductStepError,
    > {
        let activation = transaction.activation().clone();
        let mut reducer = crate::line_task::LineTaskActivation::default();
        let prior = transaction.frame().phase.clone();
        let closing_state = match prior {
            ProductDialoguePhase::Activating { fiber, pending } => {
                ProductDialogueClosingState::Activation { fiber, pending }
            }
            ProductDialoguePhase::Reducing { mut line_task } => {
                let view = self
                    .line_task_view(transaction.frame().content)
                    .ok_or(LineRuntimeError::UnknownTaskGroup)?;
                reducer = fail_live_line_task_group(&view, &mut line_task);
                ProductDialogueClosingState::LineTask { line_task }
            }
            ProductDialoguePhase::Publishing { line_task } => {
                ProductDialogueClosingState::LineTask { line_task }
            }
            ProductDialoguePhase::Closing(closing) => {
                transaction.frame_mut().phase = ProductDialoguePhase::Closing(closing);
                return Ok((transaction, batch));
            }
        };
        {
            let frame = transaction.frame_mut();
            frame.phase = ProductDialoguePhase::Closing(ProductDialogueClosing {
                failure: trap,
                state: closing_state,
            });
            frame.pending_content_events.clear();
            frame.pending_advance = false;
        }
        transaction.line_mut().abandon()?;
        let batch = self.prepare_line_task_commands_from(&mut transaction, reducer, batch)?;
        transaction
            .line_mut()
            .prepare_handle_unwind(&activation, false)?;
        Ok((transaction, batch))
    }

    fn resume_product_dialogue_failure_close(
        &mut self,
        mut transaction: dialogue::ProductDialogueTransaction,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let activation = transaction.activation().clone();
        let outcomes = std::mem::take(&mut transaction.frame_mut().pending_line_outcomes);
        if !outcomes.is_empty() {
            let pending = matches!(
                transaction.frame().phase,
                ProductDialoguePhase::Closing(ProductDialogueClosing {
                    state: ProductDialogueClosingState::Activation {
                        pending: Some(_),
                        ..
                    },
                    ..
                })
            );
            let reduced = if pending {
                self.resume_pending_line_operation(&mut transaction, &outcomes)
                    .map(|_| ())
            } else {
                transaction
                    .line_mut()
                    .accept_runtime_outcomes(&outcomes)
                    .map(|diagnostics| {
                        for diagnostic in diagnostics {
                            output.diagnostics.push(RuntimeDiagnostic::new(format!(
                                "dialogue cleanup after primary failure also failed: {diagnostic}"
                            )));
                        }
                    })
                    .map_err(ProductStepError::from)
            };
            if let Err(cleanup) = reduced {
                output.diagnostics.push(RuntimeDiagnostic::new(format!(
                    "dialogue cleanup after primary failure also failed: {cleanup}"
                )));
            }
        }
        if let Err(cleanup) = transaction
            .line_mut()
            .prepare_handle_unwind(&activation, false)
        {
            output.diagnostics.push(RuntimeDiagnostic::new(format!(
                "dialogue cleanup after primary failure also failed: {cleanup}"
            )));
        }
        let batch = ProductLineTaskExecutionBatch {
            child_fibers: self.child_fibers.clone(),
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            observations: Vec::new(),
            pure_stats: None,
        };
        self.commit_product_dialogue_failure_close(transaction, batch, output)
    }

    fn commit_product_dialogue_failure_close(
        &mut self,
        mut transaction: dialogue::ProductDialogueTransaction,
        batch: ProductLineTaskExecutionBatch,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let activation = transaction.activation().clone();
        let terminal = transaction.line().failure_close_ready()
            && !batch.has_joined_dialogue_work(&activation);
        if terminal {
            if let Err(error) = transaction.line_mut().release_frame() {
                self.fail_with_error(error.into(), output);
                return true;
            }
            let failure = match &transaction.frame().phase {
                ProductDialoguePhase::Closing(closing) => closing.failure.clone(),
                ProductDialoguePhase::Activating { .. }
                | ProductDialoguePhase::Reducing { .. }
                | ProductDialoguePhase::Publishing { .. } => {
                    self.fail_with_error(LineRuntimeError::InvalidResultTransition.into(), output);
                    return true;
                }
            };
            let receipt = match self.dialogues.commit_abandoned(transaction) {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.fail_with_error(error.into(), output);
                    return true;
                }
            };
            self.commit_line_task_commands(batch, output);
            let commands = receipt.into_line().into_commands();
            output.requests.line_commands.extend(commands);
            self.terminate_with_trap(failure, output);
            true
        } else {
            let receipt = match self.dialogues.commit(transaction) {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.fail_with_error(error.into(), output);
                    return true;
                }
            };
            self.commit_line_task_commands(batch, output);
            let commands = receipt.into_line().into_commands();
            output.requests.line_commands.extend(commands);
            false
        }
    }

    fn line_task_view(&self, content: AwbcContentUnitId) -> Option<AwbcLineTaskPlanView<'_>> {
        let group = self
            .dialogue_group(content)
            .and_then(|group| self.program.line_task_groups.get(group.index()))?;
        AwbcLineTaskPlanView::new(&self.program, group)
    }

    fn progress_line_task(
        &self,
        active: &mut ActiveDialogue,
        content_events: &AcceptedLineTaskContentEvents,
    ) -> Result<crate::line_task::LineTaskActivation, ProductStepError> {
        let elapsed_nanos = active.elapsed_nanos;
        let activation = {
            let view = self
                .line_task_view(active.content)
                .ok_or(crate::line_task::LineRuntimeError::UnknownTaskGroup)?;
            let state = active
                .line_task_mut()
                .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
            progress_live_line_task_group(
                &view,
                LogicalDuration::from_nanos(elapsed_nanos),
                content_events.ready(),
                state,
            )?
        };
        Ok(activation)
    }

    fn cancel_line_task(
        &self,
        active: &mut ActiveDialogue,
        marks: &BTreeSet<crate::runtime_id::RuntimeDialogueMarkId>,
    ) -> Result<
        (
            Option<crate::runtime_id::RuntimeDialogueMarkId>,
            crate::line_task::LineTaskActivation,
        ),
        ProductStepError,
    > {
        let (selected, commands) = {
            let view = self
                .line_task_view(active.content)
                .ok_or(crate::line_task::LineRuntimeError::UnknownTaskGroup)?;
            let state = active
                .line_task_mut()
                .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
            let selected = view.cancellation_mark(marks);
            let activation = cancel_live_line_task_group(&view, marks, state);
            let selected = activation.as_ref().and(selected);
            (selected, activation.unwrap_or_default())
        };
        Ok((selected, commands))
    }

    fn finish_line_task(
        &self,
        active: &mut ActiveDialogue,
    ) -> Result<crate::line_task::LineTaskActivation, ProductStepError> {
        let activation = {
            let view = self
                .line_task_view(active.content)
                .ok_or(crate::line_task::LineRuntimeError::UnknownTaskGroup)?;
            let state = active
                .line_task_mut()
                .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
            finish_live_line_task_group(&view, state)
        };
        Ok(activation)
    }

    fn prepare_owned_line_task_completion(
        &self,
        transaction: &mut dialogue::ProductDialogueTransaction,
        content: AwbcContentUnitId,
        tag: LineTaskWorkTag,
        child: &mut FiberState,
        failed: bool,
        cancelled: bool,
        joined: bool,
        batch: ProductLineTaskExecutionBatch,
    ) -> Result<ProductLineTaskExecutionBatch, ProductStepError> {
        if transaction.frame().content != content {
            return Err(ProductStepError::StaleLineTaskChildContent {
                expected: transaction.frame().content,
                actual: content,
            });
        }
        if let Some(token) = tag.scheduled_token().cloned() {
            let live = line::product_fiber_handle_owners(self.facade_fiber.execution, child)?
                .into_keys()
                .collect::<BTreeSet<_>>();
            let locals = transaction.line().scheduled_child_locals(&token)?;
            let values = child.take_function_argument_values(&self.program)?;
            if values.len() != locals.len() {
                return Err(
                    crate::line_task::LineRuntimeError::InvalidScheduledCaptureGraph.into(),
                );
            }
            let returned_bindings = locals
                .into_vec()
                .into_iter()
                .zip(values)
                .filter_map(|(local, value)| {
                    value.map(|value| RuntimeLocalBinding { local, value })
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let mut returned = BTreeSet::new();
            for binding in &returned_bindings {
                let handles = line::unique_line_handles(&binding.value)?;
                returned.extend(handles.into_iter().map(|handle| handle.token().clone()));
            }
            let terminal = if failed {
                crate::line_task::RuntimeScheduledState::Failed
            } else if cancelled {
                crate::line_task::RuntimeScheduledState::Cancelled
            } else {
                crate::line_task::RuntimeScheduledState::Completed
            };
            transaction.line_mut().finish_child_scope(
                &tag,
                &live,
                &returned,
                crate::effect::RuntimeDropPolicy::Default,
            )?;
            transaction.line_mut().admit_scheduled_child_bindings(
                &token,
                returned_bindings,
                terminal,
            )?;
            transaction
                .line_mut()
                .complete_scheduled_work(&token, failed, cancelled)?;
        }
        if !joined {
            return Ok(batch);
        }
        let completion = {
            let view = self
                .line_task_view(content)
                .ok_or(crate::line_task::LineRuntimeError::UnknownTaskGroup)?;
            let state = transaction
                .frame_mut()
                .line_task_mut()
                .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
            complete_live_line_task_work(&view, state, tag, failed)
        }?;
        self.prepare_line_task_commands_from(transaction, completion, batch)
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
