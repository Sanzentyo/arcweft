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
pub mod program;
mod root;
mod runtime_id;
mod snapshot;
mod suspension;

use self::dialogue::ProductDialogueStore;
use self::execution::{
    ProductVmHost, has_host_requests, has_visible_output, input_choice_selection, run_function,
    stream_id_for,
};
use self::mapping::{MappedEffect, content_request, source_diagnostic};
use self::runtime_id::line_id_from_awbc_public_id;
pub use self::snapshot::{
    AwbcProductActiveChoiceSnapshot, AwbcProductActiveDialogueSaveSnapshot,
    AwbcProductActiveDialogueSnapshot, AwbcProductChildFiberOwnerSnapshot,
    AwbcProductChildFiberSaveSnapshot, AwbcProductChildFiberSnapshot,
    AwbcProductDialogueEffectSaveSnapshot, AwbcProductExecutorRollbackImage,
    AwbcProductExecutorSaveSnapshot, AwbcProductExecutorSnapshot,
    AwbcProductLineTaskCancelSnapshot, AwbcProductLineTaskExitPolicySnapshot,
    AwbcProductLineTaskExitSnapshot, AwbcProductLineTaskFiberPhaseSnapshot,
    AwbcProductLineTaskJoinSnapshot, AwbcProductLineTaskLiveSnapshot,
    AwbcProductLineTaskNodeStateSnapshot, AwbcProductLineTaskPhaseSnapshot,
    AwbcProductLineTaskWorkSnapshot, AwbcProductLineTaskWorkTagSnapshot,
    AwbcProductPendingHostCallSnapshot, AwbcProductSaveError, AwbcProductTaskEventKindSaveSnapshot,
    AwbcProductTaskEventSaveSnapshot,
};
use crate::awbc::fiber::{
    FiberAwaitManyInFlight, FiberAwaitManyState, FiberAwaitTarget, FiberBudget, FiberCheckpoint,
    FiberCursor, FiberDialogueContentEffectBinding, FiberState, FiberStatus, FiberSuspensionReason,
    FiberTerminalValue, FiberTrap, runtime_value_matches_type,
};
use crate::awbc::schema::{
    AwbcAwaitObserverResume, AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcEffectPlanId,
    AwbcEntryId, AwbcFunctionId, AwbcHostCallId, AwbcHostCallMode, AwbcLineTaskGroupId,
    AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger, AwbcProgram, AwbcRegisterId,
    AwbcResumePointId, AwbcStreamPlanId, AwbcTaskPlanId, AwbcTrapCode, AwbcTypeId,
};
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::awbc::vm::{
    VmExecutionContext, VmExit, VmObservation, VmStepOptions, step_with_host_context,
};
use crate::engine::{
    AwaitItemType, AwaitState, ChoiceState, FlowExit, FlowFiber, FlowFiberId, FlowFiberOwner,
    FlowFiberStatus, HostCallState,
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
use crate::runtime_id::DialogueActivationId;
use crate::step::{
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeDialogueInputActionEvent,
    RuntimeHostCallId, RuntimeHostCallMode, RuntimeHostCallRequest, RuntimeStepInput,
    RuntimeStepMode, RuntimeStepOptions, RuntimeStepOutput, RuntimeStepResult, RuntimeStepStats,
    RuntimeStepStopReason,
};
use crate::stream::{RuntimeStreamEvent, StreamRuntimeState};
use crate::task::{
    GenerationId, NeedId, NeedProducerRegistry, RuntimeNeedState, TaskEvent, TaskEventKind, TaskId,
    TaskPublicationCursor, TaskSequence, normalize_runtime_need_states, normalize_task_events,
    resolved_runtime_need_state,
};
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeCallableValue, RuntimeEnv, RuntimeFlowParameterBinding, RuntimePayload, RuntimeValue,
    runtime_sequence_values, runtime_value_label,
};
use arcweft_interaction_model::audio::{AudioCommandEnvelope, AudioDispatchId};
use arcweft_need::Need;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use thiserror::Error;

/// Executes one verified stable pure-program binding through its exact AWBC
/// function frame. Callers retain domain ownership of the program identity and
/// arguments; this boundary performs no string lookup or function fallback.
pub fn evaluate_pure_program_with_backend(
    program: &Arc<AwbcProgram>,
    pure_program: arcweft_id::runtime_program::RuntimePureProgramId,
    args: &[RuntimeValue],
    backend: &mut impl RuntimeCallBackend,
) -> Result<RuntimeValue, crate::awbc::vm::VmError> {
    let binding = program.pure_program_binding(pure_program).ok_or_else(|| {
        crate::awbc::vm::VmError::Runtime(format!(
            "missing verified AWBC pure program {pure_program}"
        ))
    })?;
    program
        .functions
        .get(binding.function.index())
        .ok_or_else(|| {
            crate::awbc::vm::VmError::Runtime(format!(
                "pure program {pure_program} references missing function {}",
                binding.function.0
            ))
        })?;
    if args.len() != binding.input_types.len() {
        return Err(crate::awbc::vm::VmError::FunctionArgumentCount {
            expected: binding.input_types.len(),
            actual: args.len(),
        });
    }
    for value in args {
        if !value.ownership().permits_copy() {
            return Err(crate::awbc::vm::VmError::Runtime(format!(
                "pure program {pure_program} input contains an affine value"
            )));
        }
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
    let mut executor = AwbcProductStepExecutor::for_program_invocation(
        Arc::clone(program),
        pure_program,
        args.to_vec(),
        GenerationId::new(0),
        1_000_000,
    )
    .map_err(|error| crate::awbc::vm::VmError::Runtime(error.into_parts().0.to_string()))?;
    let output = executor.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: crate::step::RuntimeStepBudget { max_ops: 1_000_000 },
            ..RuntimeStepOptions::default()
        },
        backend,
    );
    let (_, result) = executor
        .take_program_result()
        .map_err(|error| crate::awbc::vm::VmError::Runtime(error.to_string()))?
        .ok_or_else(|| {
            crate::awbc::vm::VmError::Runtime(format!(
                "program {pure_program} did not return: {:?}; {:?}",
                output.stop_reason, output.output.diagnostics,
            ))
        })?;
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
    #[error("program {program} input {position} cannot transfer detached custody: {source}")]
    ProgramInputCustody {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
        position: usize,
        #[source]
        source: crate::value::ownership::RuntimeDetachedValueError,
    },
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
    #[error(
        "cannot rebind product AWBC generation backwards (current {current}, requested {requested})"
    )]
    GenerationRegression { current: u64, requested: u64 },
}

#[derive(Clone, Debug, Error, PartialEq)]
pub(super) enum ProductStepError {
    #[error(transparent)]
    Vm(#[from] crate::awbc::vm::VmError),
    #[error("{0}")]
    Input(String),
    #[error("{0}")]
    Type(String),
    #[error("{0}")]
    Host(String),
    #[error("{0}")]
    Internal(String),
    #[error("AWBC activation trapped: {0:?}")]
    ActivationTrap(FiberTrap),
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
            | Self::ActivationTrap(_)
            | Self::Line(_)
            | Self::LineTaskCompletion(_)
            | Self::DialogueContentIdentityOverflow
            | Self::DialogueOccurrenceOverflow
            | Self::ChildGenerationOverflow
            | Self::DialogueLineCursorOverflow
            | Self::StaleLineTaskChildContent { .. }
            | Self::RuntimeIdentity(_)
            | Self::Fiber(_)
            | Self::Vm(_) => RuntimeDiagnosticCategory::Internal,
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
            | Self::Fiber(_)
            | Self::Vm(_) => AwbcTrapCode::InternalInvariant,
            Self::ActivationTrap(trap) => trap.code,
        }
    }
}

fn partition_drop_observation(
    observations: Vec<VmObservation>,
) -> Result<
    (
        crate::line_task::RuntimeHandleDropAuthorization,
        Vec<VmObservation>,
    ),
    crate::line_task::LineRuntimeError,
> {
    let mut authorization = crate::line_task::RuntimeHandleDropAuthorization::default();
    let mut remaining = Vec::with_capacity(observations.len());
    for observation in observations {
        match observation {
            VmObservation::Drop { policy } => {
                authorization.set_boundary(Some(policy))?;
            }
            VmObservation::DiscardedValue(value) => authorization.authorize_displaced(&value)?,
            observation => remaining.push(observation),
        }
    }
    Ok((authorization, remaining))
}

#[derive(Debug, PartialEq)]
struct ActiveDialogue {
    activation: crate::runtime_id::DialogueActivationId,
    content: AwbcContentUnitId,
    target: crate::value::RuntimeOpaqueValue,
    target_type: crate::awbc::schema::AwbcTypeId,
    line: crate::plan::RuntimeLineId,
    /// External caller inputs, retained for the activation function ABI.
    captures: Box<[RuntimeValue]>,
    /// Copyable line-task inputs after reveal: external inputs, then activation exports.
    task_inputs: Box<[RuntimeValue]>,
    values: Box<[crate::plan::RuntimeDialogueValueBinding]>,
    /// Pending callbacks are removed at their first accepted effect event.
    /// The callback value has exactly one live owner until child activation.
    effect_callbacks:
        BTreeMap<crate::runtime_id::RuntimeDialogueEffectSiteId, RuntimeCallableValue>,
    voice: crate::presentation::RuntimeDialogueVoiceState,
    result: crate::awbc::schema::AwbcDialogueResultTarget,
    phase: ProductDialoguePhase,
    elapsed_nanos: u64,
    pending_content_events: Vec<crate::step::RuntimeDialogueContentEventKind>,
    pending_advance: bool,
    pending_line_outcomes: Vec<crate::presentation::RuntimeLineHostOutcome>,
    pending_activation_host_call: Option<PendingHostCall>,
}

#[derive(Debug, PartialEq)]
enum ProductDialoguePhase {
    /// In-process move boundary while the sole activation fiber is executing.
    /// This marker carries no runtime value and must never be saved or published.
    Transitioning,
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

#[derive(Debug, PartialEq)]
struct ProductDialogueClosing {
    failure: FiberTrap,
    state: ProductDialogueClosingState,
}

#[derive(Debug, PartialEq)]
enum ProductDialogueClosingState {
    Activation {
        fiber: FiberState,
        pending: Option<ProductPendingLineOperation>,
    },
    LineTask {
        line_task: LineTaskLiveState,
    },
}

#[derive(Debug, PartialEq)]
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
            | ProductDialoguePhase::Publishing { line_task }
            | ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::LineTask { line_task },
                ..
            }) => Some(line_task),
            ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::Activation { .. },
                ..
            })
            | ProductDialoguePhase::Activating { .. }
            | ProductDialoguePhase::Transitioning => None,
        }
    }

    fn line_task_mut(&mut self) -> Option<&mut LineTaskLiveState> {
        match &mut self.phase {
            ProductDialoguePhase::Reducing { line_task }
            | ProductDialoguePhase::Publishing { line_task }
            | ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::LineTask { line_task },
                ..
            }) => Some(line_task),
            ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::Activation { .. },
                ..
            })
            | ProductDialoguePhase::Activating { .. }
            | ProductDialoguePhase::Transitioning => None,
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
    Deferred {
        content: AwbcContentUnitId,
        activation: crate::runtime_id::DialogueActivationId,
        registration: crate::runtime_id::RuntimeDeferRegistrationId,
        site: crate::runtime_id::RuntimeDeferSiteId,
    },
    ScopedDeferred {
        content: AwbcContentUnitId,
        activation: crate::runtime_id::DialogueActivationId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: crate::awbc::schema::AwbcScopeId,
        registration: crate::runtime_id::RuntimeDeferRegistrationId,
        site: crate::runtime_id::RuntimeDeferSiteId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProductDeferredChildKind {
    LineRoot {
        activation: crate::runtime_id::DialogueActivationId,
        registration: crate::runtime_id::RuntimeDeferRegistrationId,
        site: crate::runtime_id::RuntimeDeferSiteId,
    },
    Scoped {
        activation: crate::runtime_id::DialogueActivationId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: crate::awbc::schema::AwbcScopeId,
        registration: crate::runtime_id::RuntimeDeferRegistrationId,
        site: crate::runtime_id::RuntimeDeferSiteId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProductLineTaskFiberPhase {
    Active,
    Closing,
}

#[derive(Debug, PartialEq)]
struct ProductChildFiber {
    owner: ProductChildFiberOwner,
    fiber: FiberState,
    runtime_generation: GenerationId,
    pending_host_call: Option<PendingHostCall>,
}

/// Fallible realization of one reducer command batch. Child fibers and their
/// identity cursors are prepared off to the side; the dialogue registry and
/// this executor substate are committed only after every command has been
/// validated and materialized.
struct ProductLineTaskExecutionBatch {
    child_fibers: VecDeque<ProductChildFiber>,
    existing_child_actions:
        BTreeMap<crate::runtime_id::RuntimeFiberInstanceId, line::ProductExistingChildAction>,
    line_task_activations: Vec<crate::line_task::LineTaskActivation>,
    line_task_baseline: Option<Option<LineTaskLiveState>>,
    line_task_reserved_runs: Vec<line::ReservedLineRunIdentity>,
    dialogue_effect_callback_activations:
        BTreeSet<crate::runtime_id::RuntimeDialogueEffectCallbackActivationId>,
    next_generation: u64,
    next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    observations: Vec<VmObservation>,
    pure_stats: Option<crate::step::RuntimePureCallStats>,
}

impl ProductLineTaskExecutionBatch {
    fn has_joined_dialogue_work(
        &self,
        activation: &crate::runtime_id::DialogueActivationId,
        existing_children: &VecDeque<ProductChildFiber>,
        prepared: &line::PreparedLineTaskCommands,
    ) -> bool {
        if prepared.has_joined_run(activation) {
            return true;
        }
        self.child_fibers
            .iter()
            .chain(existing_children)
            .any(|child| {
                if matches!(
                    prepared.child_action(child.fiber.instance),
                    Some(
                        line::ProductExistingChildAction::CancelAndJoin
                            | line::ProductExistingChildAction::Detach
                    )
                ) {
                    return false;
                }
                matches!(
                    &child.owner,
                    ProductChildFiberOwner::LineTask { tag, policy, .. }
                        if tag.activation_id() == activation
                            && policy.join == ChildJoinPolicy::Join
                ) || matches!(
                    &child.owner,
                    ProductChildFiberOwner::Deferred {
                        activation: owner_activation,
                        ..
                    } if owner_activation == activation
                ) || matches!(
                    &child.owner,
                    ProductChildFiberOwner::ScopedDeferred {
                        activation: owner_activation,
                        ..
                    } if owner_activation == activation
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
            LineTaskWork::Cancellation(action) => self
                .group
                .cancel_handlers
                .iter()
                .find(|handler| handler.trigger == action)
                .map(|handler| handler.function),
            LineTaskWork::Cleanup(ScopeExit::Completed) => self.group.cleanup_completed,
            LineTaskWork::Cleanup(ScopeExit::Cancelled) => self.group.cleanup_cancelled,
            LineTaskWork::Cleanup(ScopeExit::Failed) => self.group.cleanup_failed,
            LineTaskWork::Defer(_) => None,
        }
    }

    fn has_mark_result_selector(&self) -> bool {
        let Some(end) = self.group.nodes.checked_end() else {
            return false;
        };
        (self.group.nodes.start..end).any(|index| {
            let Some(AwbcLineTaskNode::Child {
                trigger: AwbcLineTaskTrigger::Mark(_),
                scope,
                ..
            }) = self.program.line_task_nodes.get(index as usize)
            else {
                return false;
            };
            self.subtree_has_result_selector(*scope)
        })
    }

    fn subtree_has_result_selector(&self, root: AwbcLineTaskNodeId) -> bool {
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(node_id) = pending.pop() {
            if !visited.insert(node_id) {
                continue;
            }
            let Some(node) = self.program.line_task_nodes.get(node_id.index()) else {
                continue;
            };
            match node {
                AwbcLineTaskNode::Sequence(children)
                | AwbcLineTaskNode::Start(children)
                | AwbcLineTaskNode::Parallel { children, .. } => {
                    pending.extend(children.iter().copied());
                }
                AwbcLineTaskNode::Child { scope, .. } => pending.push(*scope),
                AwbcLineTaskNode::Action(function) => {
                    let Some(function) = self.program.functions.get(function.index()) else {
                        continue;
                    };
                    let Some(end) = function.blocks.checked_end() else {
                        continue;
                    };
                    if (function.blocks.start..end).any(|block| {
                        self.program
                            .blocks
                            .get(block as usize)
                            .is_some_and(|block| {
                                matches!(
                                    block.terminator,
                                    crate::awbc::schema::AwbcTerminator::SelectDialogueResult { .. }
                                )
                            })
                    }) {
                        return true;
                    }
                }
            }
        }
        false
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

    fn cancellation_action(
        &self,
        actions: &[arcweft_interaction_model::input::InputActionId],
    ) -> Option<arcweft_interaction_model::input::InputActionId> {
        actions.iter().find_map(|action| {
            self.group
                .cancel_handlers
                .iter()
                .any(|handler| &handler.trigger == action)
                .then(|| action.clone())
        })
    }

    fn has_cancellation_work(
        &self,
        action: &arcweft_interaction_model::input::InputActionId,
    ) -> bool {
        self.group
            .cancel_handlers
            .iter()
            .any(|handler| &handler.trigger == action)
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

/// Mutable executor state touched while a deferred child consumes external
/// events. The data-only cursors are copied, while consumed events stay owned
/// by this journal until the child step commits.
pub(super) struct DeferredChildResumeJournal {
    need_publications_before: BTreeMap<
        (
            crate::runtime_id::RuntimePersistentFiberId,
            crate::task::TaskCorrelation,
        ),
        TaskPublicationCursor,
    >,
    remaining_new_task_requests_before: usize,
    next_host_call_sequence_before: u64,
    pending_host_call_before: Option<PendingHostCall>,
    consumed_task_events: Vec<DeferredTaskEvent>,
    deferred_observations: Vec<VmObservation>,
    owner_value_transferred: bool,
    drop_policy: Option<crate::effect::RuntimeDropPolicy>,
    drop_policy_conflict: bool,
    staged_need_ready: Option<DeferredNeedReadyStage>,
    staged_host_call: Option<PreparedHostResultTake>,
}

struct DeferredTaskEvent {
    queue_index: usize,
    event: TaskEvent,
    transferred_ready: Option<(AwbcTaskPlanId, usize)>,
}

pub(super) enum DeferredNeedReadySource {
    Local(crate::task::NeedProducerReadyTakeProof),
    External {
        index: usize,
        cursor: TaskPublicationCursor,
    },
}

pub(super) struct DeferredNeedReadyStage {
    pub(super) correlation: crate::task::TaskCorrelation,
    pub(super) source: DeferredNeedReadySource,
    pub(super) resume: crate::awbc::fiber::PreparedFiberResume,
    pub(super) binding: Option<crate::awbc::vm::PreparedPatternBinding>,
}

pub(super) struct PreparedHostResultTake {
    pub(super) result_id: RuntimeHostCallId,
    pub(super) result_index: usize,
    pub(super) target: PreparedHostResultTarget,
}

pub(super) enum PreparedHostResultTarget {
    Deferred {
        child: crate::runtime_id::RuntimeFiberInstanceId,
        call: AwbcHostCallId,
        outcome: PreparedHostResultOutcome,
    },
    Activation {
        activation: DialogueActivationId,
        call: AwbcHostCallId,
        outcome: PreparedHostResultOutcome,
    },
}

pub(super) enum PreparedHostResultOutcome {
    Ready {
        destination: Option<AwbcRegisterId>,
        resume: crate::awbc::fiber::PreparedFiberResume,
    },
    Failed {
        kind: crate::step::RuntimeHostCallErrorKind,
        message: String,
    },
}

impl DeferredChildResumeJournal {
    fn capture(executor: &AwbcProductStepExecutor) -> Self {
        Self {
            need_publications_before: executor.need_publications.clone(),
            remaining_new_task_requests_before: executor.remaining_new_task_requests,
            next_host_call_sequence_before: executor.next_host_call_sequence,
            pending_host_call_before: executor.pending_host_call.clone(),
            consumed_task_events: Vec::new(),
            deferred_observations: Vec::new(),
            owner_value_transferred: false,
            drop_policy: None,
            drop_policy_conflict: false,
            staged_need_ready: None,
            staged_host_call: None,
        }
    }

    fn rollback(self, executor: &mut AwbcProductStepExecutor) {
        executor.need_publications = self.need_publications_before;
        executor.remaining_new_task_requests = self.remaining_new_task_requests_before;
        executor.next_host_call_sequence = self.next_host_call_sequence_before;
        executor.pending_host_call = self.pending_host_call_before;
        for entry in self.consumed_task_events {
            // A transferred Ready payload has either been returned to this
            // journal before rollback, or remains in the live candidate
            // fiber when an invariant failure prevents checkpoint restore.
            // Its metadata-only placeholder must never be re-enqueued.
            if entry.transferred_ready.is_none() {
                executor.queued_task_events.insert(
                    entry.queue_index.min(executor.queued_task_events.len()),
                    entry.event,
                );
            }
        }
    }

    pub(super) fn stage_need_ready(
        &mut self,
        stage: DeferredNeedReadyStage,
    ) -> Result<(), ProductStepError> {
        if self.staged_need_ready.is_some() || self.staged_host_call.is_some() {
            return Err(ProductStepError::Internal(
                "deferred child already has a staged Need Ready transfer".to_owned(),
            ));
        }
        self.staged_need_ready = Some(stage);
        Ok(())
    }

    fn has_staged_need_ready(&self) -> bool {
        self.staged_need_ready.is_some()
    }

    fn take_staged_need_ready(&mut self) -> Option<DeferredNeedReadyStage> {
        self.staged_need_ready.take()
    }

    pub(super) fn stage_host_call(
        &mut self,
        stage: PreparedHostResultTake,
    ) -> Result<(), ProductStepError> {
        if self.staged_host_call.is_some() || self.staged_need_ready.is_some() {
            return Err(ProductStepError::Internal(
                "deferred child already has a staged host or Need result".to_owned(),
            ));
        }
        self.staged_host_call = Some(stage);
        Ok(())
    }

    fn has_staged_host_call(&self) -> bool {
        self.staged_host_call.is_some()
    }

    fn take_staged_host_call(&mut self) -> Option<PreparedHostResultTake> {
        self.staged_host_call.take()
    }

    fn commit_staged_need_ready(
        &mut self,
        need_producers: &mut NeedProducerRegistry,
        need_states: &mut Vec<RuntimeNeedState>,
        program: &AwbcProgram,
        fiber: &mut FiberState,
    ) {
        let Some(stage) = self.take_staged_need_ready() else {
            return;
        };
        let payload = match stage.source {
            DeferredNeedReadySource::Local(proof) => {
                need_producers.take_ready_for_need_prepared(proof)
            }
            DeferredNeedReadySource::External { index, cursor } => {
                let state = need_states.remove(index);
                let (correlation, actual_cursor, publication) = state.into_parts();
                assert_eq!(
                    correlation, stage.correlation,
                    "prepared Need correlation remains unchanged"
                );
                assert_eq!(
                    actual_cursor,
                    Some(cursor),
                    "prepared Need cursor remains unchanged"
                );
                let Need::Ready(crate::task::RuntimeNeedOutcome::Value(payload)) = publication
                else {
                    unreachable!("staged external Need Ready remains in its owned input slot")
                };
                payload
            }
        };
        if let Some(binding) = stage.binding {
            crate::awbc::vm::bind_pattern_owned_prepared(
                program,
                fiber,
                binding,
                payload.into_value(),
            );
        } else {
            drop(payload);
        }
        fiber.resume_at_prepared(stage.resume);
        self.record_owner_value_transfer();
    }

    fn commit_staged_host_call(
        &mut self,
        host_results: &mut Vec<crate::step::RuntimeHostCallResult>,
        child: &mut ProductChildFiber,
    ) {
        let Some(stage) = self.take_staged_host_call() else {
            return;
        };
        let PreparedHostResultTarget::Deferred {
            child: expected_child,
            call: expected_call,
            outcome,
        } = stage.target
        else {
            unreachable!("dialogue activation HostResult is not stored in a child journal")
        };
        assert_eq!(
            child.fiber.instance, expected_child,
            "staged HostCall result must commit to its preflighted child"
        );
        assert_eq!(
            child.pending_host_call.as_ref().map(|pending| pending.call),
            Some(expected_call),
            "staged HostCall result must match the child's pending call"
        );
        assert_eq!(
            host_results[stage.result_index].id, stage.result_id,
            "staged HostCall result row must remain at its preflighted index"
        );
        let result = host_results.remove(stage.result_index);
        match outcome {
            PreparedHostResultOutcome::Ready {
                destination,
                resume,
            } => {
                let value = result
                    .outcome
                    .expect("staged successful HostCall remains successful")
                    .into_value();
                if let Some(destination) = destination {
                    child
                        .fiber
                        .active_frame_mut()
                        .expect("preflighted HostCall frame remains active")
                        .set_register(destination, value)
                        .expect("preflighted HostCall destination remains vacant and typed");
                } else {
                    drop(value);
                }
                child.fiber.resume_at_prepared(resume);
            }
            PreparedHostResultOutcome::Failed { kind, message } => {
                let code = match kind {
                    crate::step::RuntimeHostCallErrorKind::UnsupportedCapability => {
                        AwbcTrapCode::CapabilityDenied
                    }
                    crate::step::RuntimeHostCallErrorKind::Rejected
                    | crate::step::RuntimeHostCallErrorKind::Failed => {
                        AwbcTrapCode::HostAbiMismatch
                    }
                };
                child.pending_host_call = None;
                child.fiber.mark_trapped(FiberTrap {
                    code,
                    message: Some(message),
                    source_map: None,
                });
            }
        }
        child.pending_host_call = None;
        self.record_owner_value_transfer();
    }

    pub(super) fn record_consumed_task_event(&mut self, index: usize, event: TaskEvent) {
        self.consumed_task_events.push(DeferredTaskEvent {
            queue_index: index,
            event,
            transferred_ready: None,
        });
    }

    pub(super) fn task_event(&self, record: usize) -> Option<&TaskEvent> {
        self.consumed_task_events
            .get(record)
            .map(|entry| &entry.event)
    }

    pub(super) fn take_ready_value(
        &mut self,
        record: usize,
        plan: AwbcTaskPlanId,
        item_index: usize,
    ) -> Result<RuntimeValue, ProductStepError> {
        let entry = self.consumed_task_events.get_mut(record).ok_or_else(|| {
            ProductStepError::Internal("deferred task event is absent".to_owned())
        })?;
        if entry.transferred_ready.is_some() {
            return Err(ProductStepError::Internal(
                "deferred task Ready payload was already transferred".to_owned(),
            ));
        }
        let kind = std::mem::replace(&mut entry.event.kind, TaskEventKind::Cancelled);
        match kind {
            TaskEventKind::Ready(value) => {
                entry.transferred_ready = Some((plan, item_index));
                self.record_owner_value_transfer();
                Ok(value.into_value())
            }
            other => {
                entry.event.kind = other;
                Err(ProductStepError::Internal(
                    "deferred task event is not a Ready payload".to_owned(),
                ))
            }
        }
    }

    pub(super) fn restore_ready_values(
        &mut self,
        fiber: &mut FiberState,
    ) -> Result<(), ProductStepError> {
        let transfers = self
            .consumed_task_events
            .iter()
            .enumerate()
            .filter_map(|(event_index, entry)| {
                entry
                    .transferred_ready
                    .map(|(plan, item_index)| (event_index, plan, item_index))
            })
            .collect::<Vec<_>>();
        if transfers.is_empty() {
            return Ok(());
        }
        let mut result_indices = BTreeSet::new();
        let state = match fiber
            .suspension
            .as_ref()
            .map(|suspension| &suspension.reason)
        {
            Some(FiberSuspensionReason::AwaitMany(state)) => state,
            _ => {
                return Err(ProductStepError::Internal(
                    "deferred AwaitMany payload moved without an AwaitMany suspension".to_owned(),
                ));
            }
        };
        for (_, plan, item_index) in &transfers {
            if state.plan != *plan
                || !result_indices.insert(*item_index)
                || state.results.get(*item_index).is_none_or(Option::is_none)
            {
                return Err(ProductStepError::Internal(
                    "deferred AwaitMany payload cannot be reclaimed from its result slot"
                        .to_owned(),
                ));
            }
        }
        let Some(suspension) = fiber.suspension.as_mut() else {
            unreachable!("AwaitMany suspension was checked above")
        };
        let FiberSuspensionReason::AwaitMany(state) = &mut suspension.reason else {
            unreachable!("AwaitMany suspension was checked above")
        };
        for (event_index, _, item_index) in transfers {
            let value = state.results[item_index]
                .take()
                .expect("result slot was checked before moving any Ready owner");
            let entry = &mut self.consumed_task_events[event_index];
            entry.event.kind = TaskEventKind::Ready(RuntimePayload::from(value));
            entry.transferred_ready = None;
        }
        Ok(())
    }

    pub(super) fn record_owner_value_transfer(&mut self) {
        self.owner_value_transferred = true;
    }

    pub(super) fn record_drop_policy(&mut self, policy: crate::effect::RuntimeDropPolicy) {
        if self.drop_policy.is_some_and(|current| current != policy) {
            self.drop_policy_conflict = true;
        } else {
            self.drop_policy = Some(policy);
        }
        self.owner_value_transferred = true;
    }

    pub(super) fn record_default_drop_policy(&mut self) {
        self.record_drop_policy(crate::effect::RuntimeDropPolicy::Default);
    }

    fn merge_drop_authorization(
        &self,
        mut existing: crate::line_task::RuntimeHandleDropAuthorization,
    ) -> Result<crate::line_task::RuntimeHandleDropAuthorization, ProductStepError> {
        if self.drop_policy_conflict {
            return Err(ProductStepError::Internal(
                "one deferred child step produced conflicting affine drop policies".to_owned(),
            ));
        }
        existing.set_boundary(self.drop_policy)?;
        Ok(existing)
    }

    fn has_owner_value_transfer(&self) -> bool {
        self.owner_value_transferred
    }

    pub(super) fn record_deferred_observations(
        &mut self,
        observations: impl IntoIterator<Item = VmObservation>,
    ) {
        self.deferred_observations.extend(observations);
    }

    fn commit(
        self,
        executor: &mut AwbcProductStepExecutor,
        mut staged_output: RuntimeStepOutput,
        output: &mut RuntimeStepOutput,
    ) {
        executor.consume_observations(self.deferred_observations, &mut staged_output);
        append_step_output(output, staged_output);
    }
}

fn append_step_output(output: &mut RuntimeStepOutput, mut staged: RuntimeStepOutput) {
    output.diagnostics.append(&mut staged.diagnostics);
    output.flow_events.append(&mut staged.flow_events);
    output.effects.line.append(&mut staged.effects.line);
    output
        .effects
        .stream_events
        .append(&mut staged.effects.stream_events);
    output.requests.tasks.append(&mut staged.requests.tasks);
    output.requests.audio.append(&mut staged.requests.audio);
    output
        .requests
        .cancel_scopes
        .append(&mut staged.requests.cancel_scopes);
    output
        .requests
        .ensure_content
        .append(&mut staged.requests.ensure_content);
    output
        .requests
        .host_calls
        .append(&mut staged.requests.host_calls);
    output
        .requests
        .line_commands
        .append(&mut staged.requests.line_commands);
    output
        .requests
        .root_events_next_step
        .append(&mut staged.requests.root_events_next_step);
    output.root_transitions.append(&mut staged.root_transitions);
    output.root_commands.append(&mut staged.root_commands);
}

pub(super) fn take_runtime_need_state(
    states: &mut Vec<RuntimeNeedState>,
    correlation: &crate::task::TaskCorrelation,
    cursor: TaskPublicationCursor,
) -> Option<(usize, RuntimeNeedState)> {
    let index = states
        .iter()
        .position(|state| &state.correlation == correlation && state.cursor == Some(cursor))?;
    Some((index, states.remove(index)))
}

/// Product-only presentation state derived from the compact fiber.
/// Evaluated await-many state remains in AWBC coordinates and never becomes a
/// synthetic plan-qualified expression.
#[derive(Debug, PartialEq)]
enum AwbcProductExecutorStatus {
    Shared(Box<FlowFiberStatus>),
    WaitingMany(FiberAwaitManyState),
}

/// Stateful canonical AWBC executor exposed through `RuntimeStepResult`.
#[derive(Debug, PartialEq)]
pub struct AwbcProductStepExecutor {
    pub(super) program: Arc<AwbcProgram>,
    artifact_fingerprint: crate::effect::RuntimeArtifactFingerprint,
    format_context: crate::value::RuntimeFormatContext,
    plain_text_context_template_proof:
        Option<crate::value::RuntimeDialoguePlainTextContextTemplateProof>,
    fiber: FiberState,
    /// Generation used by future producer admissions. Existing fibers and
    /// launches retain their own creation generation across compatible swaps.
    runtime_generation: GenerationId,
    facade_fiber: FlowFiber,
    entry_bound: bool,
    dialogues: ProductDialogueStore,
    active_choice: Option<ActiveChoice>,
    pending_host_call: Option<PendingHostCall>,
    started_tasks: BTreeSet<TaskId>,
    task_publications: BTreeMap<TaskId, TaskPublicationCursor>,
    need_publications: BTreeMap<
        (
            crate::runtime_id::RuntimePersistentFiberId,
            crate::task::TaskCorrelation,
        ),
        TaskPublicationCursor,
    >,
    need_producers: NeedProducerRegistry,
    remaining_new_task_requests: usize,
    queued_task_events: VecDeque<TaskEvent>,
    emitted_content: BTreeSet<AwbcContentUnitId>,
    stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    child_fibers: VecDeque<ProductChildFiber>,
    dialogue_effect_callback_activations:
        BTreeSet<crate::runtime_id::RuntimeDialogueEffectCallbackActivationId>,
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
    /// Selects the ambient locale for formatter attempts started by later steps.
    pub fn set_format_context(&mut self, context: crate::value::RuntimeFormatContext) {
        self.format_context = context;
    }

    #[must_use]
    pub const fn format_context(&self) -> &crate::value::RuntimeFormatContext {
        &self.format_context
    }

    #[must_use]
    pub const fn runtime_generation(&self) -> GenerationId {
        self.runtime_generation
    }

    /// Rebinds only the generation used for future Need producer starts.
    /// Existing fibers, saved await state, and admitted launches keep their
    /// original identities. Repeating the current pin is an idempotent no-op.
    pub fn rebind_generation(
        &mut self,
        generation: GenerationId,
    ) -> Result<(), AwbcProductStepBuildError> {
        if generation < self.runtime_generation {
            return Err(AwbcProductStepBuildError::GenerationRegression {
                current: self.runtime_generation.get(),
                requested: generation.get(),
            });
        }
        self.runtime_generation = generation;
        Ok(())
    }

    /// Returns the authoritative generation for a locally produced Need task.
    /// The task identifier is only a lookup key; generation comes from the
    /// accepted producer registry record, never from parsing its spelling.
    #[must_use]
    pub fn need_producer_generation_for_task(&self, task: &TaskId) -> Option<GenerationId> {
        self.need_producers.generation_for_task(task)
    }

    #[must_use]
    pub fn task_generation(&self, task: &TaskId) -> Option<GenerationId> {
        self.need_producer_generation_for_task(task)
    }

    pub(super) fn awbc_type_id_for_semantic_identity(
        &self,
        identity: crate::pattern::RuntimeSemanticTypeId,
    ) -> Option<AwbcTypeId> {
        let mut matches = self
            .program
            .runtime_types
            .iter()
            .enumerate()
            .filter(|(_, runtime_type)| runtime_type.semantic_identity() == identity);
        let index = matches.next()?.0;
        if matches.next().is_some() {
            return None;
        }
        u32::try_from(index).ok().map(AwbcTypeId)
    }

    /// Returns active Restartable Need producer requests, including restored
    /// rows which still need their exact TaskSpec re-issued once.
    #[must_use]
    pub fn restartable_dispatches(&self) -> Vec<crate::task::RuntimeNeedProducerDispatch> {
        self.need_producers.restartable_dispatches()
    }

    pub(crate) fn admit_host_call(
        &mut self,
        start: crate::step::RuntimeHostCallStart,
    ) -> Result<crate::step::RuntimeHostCallRequest, crate::task::NeedProducerAdmissionError> {
        crate::step::RuntimeHostCallRequest::admit_start(
            start,
            self.runtime_generation,
            &mut self.need_producers,
        )
    }

    /// Needs whose active producer contract requires the host task to finish
    /// before this Product state can cross a save boundary.
    #[must_use]
    pub fn quiescence_blocking_needs(&self) -> Vec<NeedId> {
        self.need_producers
            .launches()
            .filter(|launch| {
                launch.restart() == crate::task::HostRestartPolicy::MustBeQuiescent
                    && !launch.task_terminal()
                    && !launch.state().is_terminal()
                    && launch.task_fault().is_none()
            })
            .map(|launch| launch.need().clone())
            .collect()
    }

    /// Captures a complete save-safe Product snapshot. A live
    /// MustBeQuiescent producer is reported as a typed deferral; Sans-I/O
    /// Product code never waits for a host task.
    pub fn snapshot_for_save(&self) -> Result<AwbcProductExecutorSnapshot, AwbcProductSaveError> {
        let needs = self.quiescence_blocking_needs();
        if !needs.is_empty() {
            return Err(AwbcProductSaveError::NeedsQuiescence { needs });
        }
        let snapshot = self.snapshot()?;
        self.validate_snapshot(&snapshot).map_err(|error| {
            AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            }
        })?;
        Ok(snapshot)
    }

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
        self.replace_program_preserving_state_arc(Arc::new(program))
    }

    /// Rebinds the exact generation lease shared with upper-layer consumers.
    pub fn replace_program_preserving_state_arc(
        &mut self,
        program: Arc<AwbcProgram>,
    ) -> Result<(), AwbcProductStepBuildError> {
        self.replace_program_preserving_state_arc_with_context_proof(program, None)
    }

    /// Rebinds to a new verified AWBC program and its bundle-certified
    /// plain-text context template proof.
    pub fn replace_program_preserving_state_arc_with_plain_text_context_proof(
        &mut self,
        program: Arc<AwbcProgram>,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
    ) -> Result<(), AwbcProductStepBuildError> {
        self.replace_program_preserving_state_arc_with_context_proof(program, Some(proof))
    }

    fn replace_program_preserving_state_arc_with_context_proof(
        &mut self,
        program: Arc<AwbcProgram>,
        proof: Option<crate::value::RuntimeDialoguePlainTextContextTemplateProof>,
    ) -> Result<(), AwbcProductStepBuildError> {
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        let template = program
            .validated_plain_text_context_template()
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        if proof.is_some_and(|proof| template.is_none_or(|reference| !proof.matches_ref(reference)))
        {
            return Err(AwbcProductStepBuildError::InvalidProgram {
                message: "plain-text context proof does not match the AWBC program pointer"
                    .to_owned(),
            });
        }
        let fingerprint = Self::artifact_fingerprint(&program)?;
        let snapshot =
            self.snapshot()
                .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                })?;
        let previous_program = std::mem::replace(&mut self.program, program);
        let previous_proof = std::mem::replace(&mut self.plain_text_context_template_proof, proof);
        let observations = match self.validate_snapshot(&snapshot) {
            Ok(observations) => observations,
            Err(error) => {
                self.program = previous_program;
                self.plain_text_context_template_proof = previous_proof;
                return Err(error);
            }
        };
        self.facade_fiber.observations = observations;
        self.artifact_fingerprint = fingerprint;
        self.rebuild_facade_stream_states_from_compact();
        self.sync_facade();
        Ok(())
    }

    pub fn for_entry(
        program: AwbcProgram,
        entry: AwbcEntryId,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::for_entry_arc(Arc::new(program), entry, budget_quantum)
    }

    /// Starts against the same immutable program lease used by its generation.
    pub fn for_entry_arc(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::for_entry_arc_with_generation(program, entry, budget_quantum, GenerationId::new(0))
    }

    /// Starts an AWBC executor pinned to the owning runtime generation.
    pub fn for_entry_arc_with_generation(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
        budget_quantum: u64,
        generation: GenerationId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::for_entry_arc_with_context_proof(program, entry, budget_quantum, generation, None)
    }

    /// Starts against a bundle-certified context-template proof. Standalone
    /// AWBC callers use [`Self::for_entry_arc`], which leaves String context
    /// conversion unavailable.
    pub fn for_entry_arc_with_plain_text_context_proof(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
        budget_quantum: u64,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::for_entry_arc_with_context_proof(
            program,
            entry,
            budget_quantum,
            GenerationId::new(0),
            Some(proof),
        )
    }

    pub fn for_entry_arc_with_plain_text_context_proof_and_generation(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
        budget_quantum: u64,
        generation: GenerationId,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::for_entry_arc_with_context_proof(
            program,
            entry,
            budget_quantum,
            generation,
            Some(proof),
        )
    }

    fn for_entry_arc_with_context_proof(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
        budget_quantum: u64,
        generation: GenerationId,
        proof: Option<crate::value::RuntimeDialoguePlainTextContextTemplateProof>,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::for_root_arc_with_context_proof(
            program,
            crate::awbc::fiber::AwbcFiberRoot::Entry(entry),
            budget_quantum,
            generation,
            proof,
        )
    }

    fn for_root_arc_with_context_proof(
        program: Arc<AwbcProgram>,
        origin: crate::awbc::fiber::AwbcFiberRoot,
        budget_quantum: u64,
        generation: GenerationId,
        proof: Option<crate::value::RuntimeDialoguePlainTextContextTemplateProof>,
    ) -> Result<Self, AwbcProductStepBuildError> {
        program
            .verify(
                AwbcVerifyBudget::default(),
                AwbcVerifyContext {
                    require_entrypoint: origin.entry().is_some(),
                    ..AwbcVerifyContext::default()
                },
            )
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        let template = program
            .validated_plain_text_context_template()
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        if proof.is_some_and(|proof| template.is_none_or(|reference| !proof.matches_ref(reference)))
        {
            return Err(AwbcProductStepBuildError::InvalidProgram {
                message: "plain-text context proof does not match the AWBC program pointer"
                    .to_owned(),
            });
        }
        let mut root_startup = match origin.entry() {
            Some(entry) => root::prepare_startup(&program, entry)?,
            None => None,
        };
        let mut fiber = if origin == crate::awbc::fiber::AwbcFiberRoot::Empty {
            if !program.entries.is_empty() {
                return Err(AwbcProductStepBuildError::FiberState {
                    message: "empty fiber origin cannot own a program with entries".to_owned(),
                });
            }
            FiberState {
                instance: crate::runtime_id::RuntimeFiberInstanceId::from_allocated(
                    std::num::NonZeroU64::MIN,
                ),
                next_frame_instance: crate::runtime_id::RuntimeIdCursor::initial(),
                generation: generation.get(),
                root: crate::awbc::fiber::AwbcFiberRoot::Empty,
                cursor: FiberCursor {
                    function: AwbcFunctionId::default(),
                    block: AwbcBlockId::default(),
                    instruction_offset: 0,
                },
                frames: Vec::new(),
                status: FiberStatus::Returned,
                suspension: None,
                terminal: Some(FiberTerminalValue::Returned(None)),
                return_summary: None,
                budget: FiberBudget {
                    remaining: budget_quantum,
                    quantum: budget_quantum,
                },
                line_cursor: 0,
                streams: Vec::new(),
            }
        } else {
            let fiber = match origin {
                crate::awbc::fiber::AwbcFiberRoot::Entry(entry) => {
                    FiberState::for_entry(&program, entry, generation.get(), budget_quantum.max(1))
                }
                crate::awbc::fiber::AwbcFiberRoot::Program(id) => program
                    .pure_program_binding(id)
                    .ok_or(crate::awbc::fiber::FiberStateError::UnknownProgram(id))
                    .and_then(|binding| {
                        FiberState::for_function(
                            &program,
                            origin,
                            binding.function,
                            generation.get(),
                            budget_quantum.max(1),
                        )
                    }),
                crate::awbc::fiber::AwbcFiberRoot::Function(function) => FiberState::for_function(
                    &program,
                    origin,
                    function,
                    generation.get(),
                    budget_quantum.max(1),
                ),
                crate::awbc::fiber::AwbcFiberRoot::Empty => {
                    unreachable!("empty origin is handled above")
                }
            };
            fiber.map_err(|error| AwbcProductStepBuildError::FiberState {
                message: error.to_string(),
            })?
        };
        root::bind_startup(&program, &mut fiber, root_startup.as_ref())?;
        if origin.entry().is_some() && root_startup.is_none() && !fiber.frames.is_empty() {
            fiber
                .bind_flow_parameter_coordinates(&program, &[])
                .map_err(|error| AwbcProductStepBuildError::FiberState {
                    message: error.to_string(),
                })?;
        }
        let artifact_fingerprint = Self::artifact_fingerprint(&program)?;
        let mut executor = Self::for_fiber(program, fiber, artifact_fingerprint, generation);
        executor.plain_text_context_template_proof = proof;
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
        Self::for_function_invocation_with_generation(
            program,
            entry,
            function,
            bindings,
            budget_quantum,
            GenerationId::new(0),
        )
    }

    pub fn for_function_invocation_with_generation(
        program: AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        bindings: impl IntoIterator<Item = RuntimeFlowParameterBinding>,
        budget_quantum: u64,
        generation: GenerationId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        let program = Arc::new(program);
        let mut fiber = FiberState::for_entry_target_function(
            &program,
            entry,
            function,
            generation.get(),
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
        let mut executor = Self::for_fiber(program, fiber, artifact_fingerprint, generation);
        executor.entry_bound = true;
        Ok(executor)
    }

    fn for_fiber(
        program: Arc<AwbcProgram>,
        fiber: FiberState,
        artifact_fingerprint: crate::effect::RuntimeArtifactFingerprint,
        runtime_generation: GenerationId,
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
            selected_dialogue_result: None,
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
            format_context: crate::value::RuntimeFormatContext::default(),
            plain_text_context_template_proof: None,
            fiber,
            runtime_generation,
            facade_fiber,
            entry_bound: false,
            dialogues: ProductDialogueStore::default(),
            active_choice: None,
            pending_host_call: None,
            started_tasks: BTreeSet::new(),
            task_publications: BTreeMap::new(),
            need_publications: BTreeMap::new(),
            need_producers: NeedProducerRegistry::default(),
            remaining_new_task_requests: usize::MAX,
            queued_task_events: VecDeque::new(),
            emitted_content: BTreeSet::new(),
            stream_sequences: BTreeMap::new(),
            child_fibers: VecDeque::new(),
            dialogue_effect_callback_activations: BTreeSet::new(),
            dialogue_occurrences: BTreeMap::new(),
            next_generation: 1,
            next_fiber_instance,
            next_host_call_sequence: 0,
            next_audio_sequence: 0,
            compact_pure_stats: crate::step::RuntimePureCallStats::default(),
            root: None,
        }
    }

    pub fn program(&self) -> &AwbcProgram {
        &self.program
    }

    /// Retains the selected executable type authority across host completion.
    pub fn program_arc(&self) -> Arc<AwbcProgram> {
        Arc::clone(&self.program)
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.facade_fiber
    }

    pub const fn compact_fiber(&self) -> &FiberState {
        &self.fiber
    }

    fn prepare_dialogue_effect_callback(
        &self,
        callback: RuntimeCallableValue,
        prepared: crate::awbc::fiber::PreparedCallableCallbackActivation,
        instance: crate::runtime_id::RuntimeFiberInstanceId,
    ) -> ProductChildFiber {
        let fiber = FiberState::for_callable_callback_prepared(
            &self.program,
            callback,
            prepared,
            instance,
            self.runtime_generation.get(),
            self.fiber.budget.quantum.max(1),
        );
        ProductChildFiber {
            owner: ProductChildFiberOwner::Independent,
            fiber,
            runtime_generation: self.runtime_generation,
            pending_host_call: None,
        }
    }

    fn stage_dialogue_effect_callbacks(
        &self,
        batch: &mut ProductLineTaskExecutionBatch,
        transaction: &mut dialogue::ProductDialogueTransaction,
        sites: &[crate::runtime_id::RuntimeDialogueEffectSiteId],
    ) -> Result<(), ProductStepError> {
        let activation = transaction.activation().clone();
        let mut seen = BTreeSet::new();
        let mut next_fiber_instance = batch.next_fiber_instance;
        let mut prepared = Vec::with_capacity(sites.len());
        for site in sites {
            let key = crate::runtime_id::RuntimeDialogueEffectCallbackActivationId::new(
                activation.clone(),
                *site,
            );
            if !seen.insert(*site) || batch.dialogue_effect_callback_activations.contains(&key) {
                return Err(ProductStepError::Input(format!(
                    "dialogue effect callback activation was already reserved: {key:?}"
                )));
            }
            let callback = transaction
                .frame()
                .effect_callbacks
                .get(site)
                .ok_or_else(|| {
                    ProductStepError::Input(format!(
                        "dialogue effect site {site} has no stored callback"
                    ))
                })?;
            let proof =
                crate::awbc::fiber::validate_runtime_callable_activation(&self.program, callback)?;
            let instance = next_fiber_instance
                .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
                .map(crate::runtime_id::RuntimeFiberInstanceId::from_allocated)?;
            prepared.push((*site, key, proof, instance));
        }
        for (site, key, proof, instance) in prepared {
            if !batch.dialogue_effect_callback_activations.insert(key) {
                unreachable!("callback activation was preflighted")
            }
            let callback = transaction
                .frame_mut()
                .effect_callbacks
                .remove(&site)
                .expect("callback was preflighted in the owning dialogue frame");
            let child = self.prepare_dialogue_effect_callback(callback, proof, instance);
            batch.child_fibers.push_back(child);
        }
        batch.next_fiber_instance = next_fiber_instance;
        Ok(())
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
        backend.set_format_context(self.format_context.clone());
        self.step_with_pure_backend(input, options, &mut backend)
    }

    pub fn step_with_pure_backend(
        &mut self,
        mut input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.remaining_new_task_requests = options.max_new_task_requests;
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
        let mut need_states = normalize_runtime_need_states(std::mem::take(&mut input.need_states));
        let mut need_index = 0;
        while need_index < need_states.len() {
            match need_states[need_index].inspect_host_ready_ownership() {
                Ok(()) => need_index += 1,
                Err(error) => {
                    need_states.remove(need_index);
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        error.to_string(),
                        None,
                        &mut output,
                    );
                }
            }
        }
        let mut host_result_index = 0;
        let mut seen_host_result_ids = BTreeSet::new();
        while host_result_index < input.host_call_results.len() {
            if !seen_host_result_ids.insert(input.host_call_results[host_result_index].id.clone()) {
                input.host_call_results.remove(host_result_index);
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    "step input contains duplicate host-call result identities".to_owned(),
                    None,
                    &mut output,
                );
                continue;
            }
            match input.host_call_results[host_result_index].inspect_host_payload_ownership() {
                Ok(()) => host_result_index += 1,
                Err(error) => {
                    input.host_call_results.remove(host_result_index);
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        error.to_string(),
                        None,
                        &mut output,
                    );
                }
            }
        }
        let task_events = normalize_task_events(std::mem::take(&mut input.task_events));
        let need_states_in = need_states.len();
        let task_events_in = task_events.len();
        Self::append_task_event_diagnostics(&mut output, &task_events);
        self.latch_task_events(task_events, &mut output);
        self.emit_pending_need_reensure(&mut output);
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
            &mut input,
            &mut need_states,
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
                event.correlation.task_id, event.cursor.sequence.0
            ))
        }));
    }

    fn run_main_work(
        &mut self,
        input: &mut RuntimeStepInput,
        need_states: &mut Vec<RuntimeNeedState>,
        output: &mut RuntimeStepOutput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> usize {
        let mut executed_ops = 0_usize;
        while executed_ops < options.budget.max_ops && self.has_attemptable_work() {
            if self.fiber.status == FiberStatus::Suspended {
                let progressed =
                    self.resume_main_suspension(input, need_states, output, pure_backend);
                executed_ops = executed_ops.saturating_add(usize::from(progressed));
                if !progressed {
                    if !self.step_next_child(output, pure_backend, input, need_states) {
                        break;
                    }
                    executed_ops = executed_ops.saturating_add(1);
                }
                if self.should_return_to_host(options.mode, output, executed_ops) {
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
            } else if !self.step_next_child(output, pure_backend, input, need_states) {
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

    fn vm_execution_context(&self) -> VmExecutionContext {
        match self.plain_text_context_template_proof {
            Some(proof) => VmExecutionContext::for_program_with_plain_text_context_proof(
                self.artifact_fingerprint,
                Arc::clone(&self.program),
                proof,
            ),
            None => VmExecutionContext::for_program(
                self.artifact_fingerprint,
                Arc::clone(&self.program),
            ),
        }
        .with_format_context(self.format_context.clone())
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
        need_states: &mut Vec<RuntimeNeedState>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> usize {
        let before_owners =
            match line::product_fiber_handle_owners(self.facade_fiber.execution, &self.fiber) {
                Ok(owners) => owners,
                Err(error) => {
                    self.fail_with_error(error, output);
                    return 1;
                }
            };
        let checkpoint = match self.fiber.checkpoint() {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                self.fail_with_error(error.into(), output);
                return 1;
            }
        };
        let mut candidate_stats = self.compact_pure_stats;
        let mut host = ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut candidate_stats,
            program_owner: crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
        };
        let context = self.vm_execution_context();
        match step_with_host_context(
            &self.program,
            &mut self.fiber,
            VmStepOptions {
                max_instructions: 1,
            },
            &context,
            &mut host,
        ) {
            Ok(mut vm_output) => {
                if matches!(
                    self.fiber.root,
                    crate::awbc::fiber::AwbcFiberRoot::Program(_)
                ) && let VmExit::Returned(value) = &mut vm_output.exit
                {
                    let Some(value) = value.take() else {
                        return self.fail_main_vm_step(
                            checkpoint,
                            ProductStepError::Internal(
                                "admitted program returned without its typed result".to_owned(),
                            ),
                            output,
                            vm_output.executed,
                        );
                    };
                    self.fiber.return_summary = Some(crate::value::runtime_value_label(&value));
                    self.fiber.terminal = Some(FiberTerminalValue::Returned(Some(value)));
                }
                let (drop_policy, observations) =
                    match partition_drop_observation(vm_output.observations) {
                        Ok(parts) => parts,
                        Err(error) => {
                            return self.fail_main_vm_step(
                                checkpoint,
                                error.into(),
                                output,
                                vm_output.executed,
                            );
                        }
                    };
                let after_owners = match line::product_fiber_handle_owners(
                    self.facade_fiber.execution,
                    &self.fiber,
                ) {
                    Ok(owners) => owners,
                    Err(error) => {
                        return self.fail_main_vm_step(
                            checkpoint,
                            error,
                            output,
                            vm_output.executed,
                        );
                    }
                };
                let drop_receipt = match self.dialogues.reconcile_parent_fiber(
                    self.facade_fiber.execution,
                    &before_owners,
                    &after_owners,
                    &drop_policy,
                ) {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        return self.fail_main_vm_step(
                            checkpoint,
                            error.into(),
                            output,
                            vm_output.executed,
                        );
                    }
                };
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
                    VmExit::Returned(value) => {
                        let retained = match &self.fiber.terminal {
                            Some(FiberTerminalValue::Returned(value)) => value.as_ref(),
                            _ => None,
                        };
                        Self::record_return(retained.or(value.as_ref()), output);
                    }
                    VmExit::DialogueResultSelected(_) => self.fail_with_error(
                        ProductStepError::Internal(
                            "dialogue result selection escaped its line-task child".to_owned(),
                        ),
                        output,
                    ),
                    VmExit::Running
                    | VmExit::Cancelled
                    | VmExit::Trapped(_)
                    | VmExit::BudgetYield(_) => {}
                }
                usize::try_from(vm_output.executed).unwrap_or(usize::MAX)
            }
            Err(error) => self.fail_main_vm_step(
                checkpoint,
                ProductStepError::Internal(error.to_string()),
                output,
                1,
            ),
        }
    }

    fn fail_main_vm_step(
        &mut self,
        checkpoint: FiberCheckpoint,
        error: ProductStepError,
        output: &mut RuntimeStepOutput,
        executed: u64,
    ) -> usize {
        let owner = crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program));
        if let Err(restore_error) = self.fiber.restore(checkpoint, &owner) {
            self.fail_with_error(ProductStepError::Fiber(restore_error), output);
        } else {
            self.fail_with_error(error, output);
        }
        usize::try_from(executed).unwrap_or(usize::MAX)
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
            match FiberState::for_function(&self.program, self.fiber.root, transform, 0, 64) {
                Ok(fiber) => fiber,
                Err(error) => {
                    self.record_error(ProductStepError::Internal(error.to_string()), output);
                    return;
                }
            };
        loop {
            let step = {
                let context = self.vm_execution_context();
                let mut host = ProductVmHost {
                    backend: pure_backend,
                    fallback_stats: &mut self.compact_pure_stats,
                    program_owner: crate::task::RuntimeProgramOwner::Awbc(Arc::clone(
                        &self.program,
                    )),
                };
                step_with_host_context(
                    &self.program,
                    &mut fiber,
                    VmStepOptions {
                        max_instructions: 64,
                    },
                    &context,
                    &mut host,
                )
            };
            match step {
                Ok(vm_output) => {
                    self.consume_observations(vm_output.observations, output);
                    match vm_output.exit {
                        VmExit::Running => {}
                        VmExit::Returned(_)
                        | VmExit::DialogueResultSelected(_)
                        | VmExit::Cancelled => return,
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

    fn rollback_selected_child_step(
        &mut self,
        mut child: ProductChildFiber,
        checkpoint: FiberCheckpoint,
        pending_host_call_before: Option<PendingHostCall>,
        mut journal: DeferredChildResumeJournal,
        _need_states: &mut Vec<RuntimeNeedState>,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if let Err(error) = journal.restore_ready_values(&mut child.fiber) {
            // Keep the live candidate (and any result owner still held in its
            // AwaitMany frame) in the executor if an internal cursor mismatch
            // prevents reconstructing its inbound event. The metadata-only
            // journal entry is intentionally not re-enqueued.
            journal.rollback(self);
            child.pending_host_call = pending_host_call_before;
            self.child_fibers.push_front(child);
            self.fail_with_error(error, output);
            return false;
        }
        journal.rollback(self);
        child.pending_host_call = pending_host_call_before;
        let owner = crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program));
        let restore = child.fiber.restore(checkpoint, &owner);
        self.child_fibers.push_front(child);
        if let Err(error) = restore {
            self.fail_with_error(error.into(), output);
            false
        } else {
            true
        }
    }

    fn step_next_child(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
        input: &mut RuntimeStepInput,
        need_states: &mut Vec<RuntimeNeedState>,
    ) -> bool {
        let Some(front) = self.child_fibers.front() else {
            return false;
        };
        let deferred_owner = matches!(
            &front.owner,
            ProductChildFiberOwner::Deferred { .. } | ProductChildFiberOwner::ScopedDeferred { .. }
        );
        if front.fiber.status != FiberStatus::Running
            && !(deferred_owner
                && matches!(
                    front.fiber.status,
                    FiberStatus::Suspended
                        | FiberStatus::Returned
                        | FiberStatus::Cancelled
                        | FiberStatus::Trapped
                ))
        {
            if let Some(child) = self.child_fibers.pop_front() {
                self.child_fibers.push_back(child);
            }
            return false;
        }
        let checkpoint = match front.fiber.checkpoint() {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                self.fail_with_error(error.into(), output);
                return true;
            }
        };
        let pending_host_call_before = front.pending_host_call.clone();
        let mut resume_journal = DeferredChildResumeJournal::capture(self);
        let mut staged_resume_output = RuntimeStepOutput::default();
        let Some(mut child) = self.child_fibers.pop_front() else {
            return false;
        };
        let owner = child.owner.clone();
        let before_handles = match &owner {
            ProductChildFiberOwner::LineTask { .. }
            | ProductChildFiberOwner::Deferred { .. }
            | ProductChildFiberOwner::ScopedDeferred { .. } => {
                match line::product_fiber_handle_tokens(self.facade_fiber.execution, &child.fiber) {
                    Ok(handles) => Some(handles),
                    Err(error) => {
                        if !self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            return true;
                        }
                        return match &owner {
                            ProductChildFiberOwner::LineTask { tag, .. } => {
                                self.begin_product_line_task_child_failure(&tag, error, output)
                            }
                            ProductChildFiberOwner::Deferred { .. } => {
                                self.fail_with_error(error.into(), output);
                                true
                            }
                            ProductChildFiberOwner::ScopedDeferred { .. } => {
                                self.fail_with_error(error.into(), output);
                                true
                            }
                            ProductChildFiberOwner::Independent => unreachable!(),
                        };
                    }
                }
            }
            ProductChildFiberOwner::Independent => None,
        };
        let mut skip_vm_instruction = false;
        if child.fiber.status == FiberStatus::Suspended {
            let request_count = output.requests.host_calls.len()
                + output.requests.tasks.len()
                + output.requests.ensure_content.len();
            match self.resume_deferred_child_suspension(
                &mut child,
                need_states,
                &mut input.host_call_results,
                &mut staged_resume_output,
                &mut resume_journal,
                pure_backend,
            ) {
                Ok(true)
                    if matches!(
                        child.fiber.status,
                        FiberStatus::Running | FiberStatus::Suspended
                    ) && (resume_journal.has_owner_value_transfer()
                        || resume_journal.has_staged_need_ready()
                        || resume_journal.has_staged_host_call()) =>
                {
                    // Finish this resume as a zero-instruction child step so
                    // line/registry preflight can commit before the next VM
                    // instruction or a staged Need Ready payload transfer.
                    skip_vm_instruction = true;
                }
                Ok(true)
                    if matches!(
                        child.fiber.status,
                        FiberStatus::Cancelled | FiberStatus::Trapped | FiberStatus::Returned
                    ) => {}
                Ok(_) => {
                    self.child_fibers.push_back(child);
                    resume_journal.commit(self, staged_resume_output, output);
                    return output.requests.host_calls.len()
                        + output.requests.tasks.len()
                        + output.requests.ensure_content.len()
                        > request_count;
                }
                Err(error) => {
                    child.fiber.mark_trapped(FiberTrap {
                        code: error.trap_code(),
                        message: Some(error.to_string()),
                        source_map: None,
                    });
                }
            }
        }
        child.fiber.replenish_budget();
        let mut candidate_stats = self.compact_pure_stats;
        let mut host = ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut candidate_stats,
            program_owner: crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
        };
        let context = self.vm_execution_context();
        let terminal_exit = matches!(
            child.fiber.status,
            FiberStatus::Returned | FiberStatus::Cancelled | FiberStatus::Trapped
        )
        .then(|| crate::awbc::vm::terminal_exit(&mut child.fiber));
        let vm_result = if skip_vm_instruction {
            Ok(crate::awbc::vm::VmStepOutput {
                executed: 0,
                exit: VmExit::Running,
                observations: Vec::new(),
            })
        } else {
            terminal_exit.map_or_else(
                || {
                    step_with_host_context(
                        &self.program,
                        &mut child.fiber,
                        VmStepOptions {
                            max_instructions: 1,
                        },
                        &context,
                        &mut host,
                    )
                },
                |exit| {
                    Ok(crate::awbc::vm::VmStepOutput {
                        executed: 0,
                        exit,
                        observations: Vec::new(),
                    })
                },
            )
        };
        let vm_output = match vm_result {
            Ok(vm_output) => vm_output,
            Err(error) => {
                let message = error.to_string();
                match &owner {
                    ProductChildFiberOwner::LineTask { tag, .. } => {
                        if !self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            return true;
                        }
                        return self.begin_product_line_task_child_failure(
                            &tag,
                            ProductStepError::Internal(message),
                            output,
                        );
                    }
                    ProductChildFiberOwner::Deferred { .. }
                    | ProductChildFiberOwner::ScopedDeferred { .. } => {
                        child.fiber.mark_trapped(FiberTrap {
                            code: AwbcTrapCode::InternalInvariant,
                            message: Some(message),
                            source_map: None,
                        });
                        crate::awbc::vm::VmStepOutput {
                            executed: 0,
                            exit: VmExit::Trapped(FiberTrap {
                                code: AwbcTrapCode::InternalInvariant,
                                message: Some("deferred child VM step failed".to_owned()),
                                source_map: None,
                            }),
                            observations: Vec::new(),
                        }
                    }
                    ProductChildFiberOwner::Independent => {
                        if !self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            return true;
                        }
                        self.fail_with_error(ProductStepError::Internal(message), output);
                        return true;
                    }
                }
            }
        };
        if let VmExit::DialogueResultSelected(value) = vm_output.exit {
            // VmExit is the first-delivery owner. The child remains the sole
            // live carrier until result-selection admission moves it to the
            // dialogue line transaction below.
            child.fiber.terminal = Some(FiberTerminalValue::DialogueResultSelected(value));
        }
        let (drop_policy, observations) = match partition_drop_observation(vm_output.observations) {
            Ok(parts) => parts,
            Err(error) => match &owner {
                ProductChildFiberOwner::LineTask { tag, .. } => {
                    if !self.rollback_selected_child_step(
                        child,
                        checkpoint,
                        pending_host_call_before,
                        resume_journal,
                        need_states,
                        output,
                    ) {
                        return true;
                    }
                    return self.begin_product_line_task_child_failure(&tag, error.into(), output);
                }
                ProductChildFiberOwner::Deferred { .. }
                | ProductChildFiberOwner::ScopedDeferred { .. } => {
                    child.fiber.mark_trapped(FiberTrap {
                        code: AwbcTrapCode::InternalInvariant,
                        message: Some(error.to_string()),
                        source_map: None,
                    });
                    (Default::default(), Vec::new())
                }
                ProductChildFiberOwner::Independent => {
                    if !self.rollback_selected_child_step(
                        child,
                        checkpoint,
                        pending_host_call_before,
                        resume_journal,
                        need_states,
                        output,
                    ) {
                        return true;
                    }
                    self.fail_with_error(error.into(), output);
                    return true;
                }
            },
        };
        let drop_policy = match resume_journal.merge_drop_authorization(drop_policy) {
            Ok(policy) => policy,
            Err(error) => {
                if !self.rollback_selected_child_step(
                    child,
                    checkpoint,
                    pending_host_call_before,
                    resume_journal,
                    need_states,
                    output,
                ) {
                    return true;
                }
                self.fail_with_error(error, output);
                return true;
            }
        };
        if matches!(&owner, ProductChildFiberOwner::Independent) {
            if matches!(
                child.fiber.status,
                FiberStatus::Running | FiberStatus::Suspended
            ) {
                self.child_fibers.push_back(child);
            }
            resume_journal.commit(self, staged_resume_output, output);
            self.compact_pure_stats = candidate_stats;
            self.consume_observations(observations, output);
            return true;
        }
        let (content, tag, policy, phase, deferred) = match owner {
            ProductChildFiberOwner::LineTask {
                content,
                tag,
                policy,
                phase,
            } => (content, tag, policy, phase, None),
            ProductChildFiberOwner::Deferred {
                content,
                activation,
                registration,
                site,
            } => (
                content,
                LineTaskWorkTag::activation(activation.clone(), LineTaskWork::Defer(registration)),
                LineTaskExitPolicy::default(),
                ProductLineTaskFiberPhase::Active,
                Some(ProductDeferredChildKind::LineRoot {
                    activation,
                    registration,
                    site,
                }),
            ),
            ProductChildFiberOwner::ScopedDeferred {
                content,
                activation,
                frame,
                scope,
                registration,
                site,
            } => (
                content,
                LineTaskWorkTag::activation(activation.clone(), LineTaskWork::Defer(registration)),
                LineTaskExitPolicy::default(),
                ProductLineTaskFiberPhase::Active,
                Some(ProductDeferredChildKind::Scoped {
                    activation,
                    frame,
                    scope,
                    registration,
                    site,
                }),
            ),
            ProductChildFiberOwner::Independent => {
                self.fail_with_error(
                    crate::line_task::LineRuntimeError::InvalidActivationOperation.into(),
                    output,
                );
                return true;
            }
        };
        if child.fiber.status == FiberStatus::Suspended && deferred.is_none() {
            if !self.rollback_selected_child_step(
                child,
                checkpoint,
                pending_host_call_before,
                resume_journal,
                need_states,
                output,
            ) {
                return true;
            }
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
                if !self.rollback_selected_child_step(
                    child,
                    checkpoint,
                    pending_host_call_before,
                    resume_journal,
                    need_states,
                    output,
                ) {
                    return true;
                }
                self.fail_with_error(error.into(), output);
                return true;
            }
        };
        let after_handles =
            match line::product_fiber_handle_tokens(self.facade_fiber.execution, &child.fiber) {
                Ok(handles) => handles,
                Err(error) => {
                    if !self.rollback_selected_child_step(
                        child,
                        checkpoint,
                        pending_host_call_before,
                        resume_journal,
                        need_states,
                        output,
                    ) {
                        let _ = self.dialogues.restore_transaction(transaction);
                        return true;
                    }
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
        let Some(before_handles) = before_handles.as_ref() else {
            if !self.rollback_selected_child_step(
                child,
                checkpoint,
                pending_host_call_before,
                resume_journal,
                need_states,
                output,
            ) {
                let _ = self.dialogues.restore_transaction(transaction);
                return true;
            }
            return self.begin_product_dialogue_failure(
                transaction,
                LineRuntimeError::InvalidActivationOperation.into(),
                output,
            );
        };
        if let Err(error) = transaction.line_mut().reconcile_child_scope_step(
            &tag,
            before_handles,
            &after_handles,
            &drop_policy,
        ) {
            if !self.rollback_selected_child_step(
                child,
                checkpoint,
                pending_host_call_before,
                resume_journal,
                need_states,
                output,
            ) {
                let _ = self.dialogues.restore_transaction(transaction);
                return true;
            }
            return self.begin_product_dialogue_failure(transaction, error.into(), output);
        }
        if !skip_vm_instruction
            && child.fiber.status == FiberStatus::Suspended
            && deferred.is_some()
            && let Err(error) =
                self.initialize_deferred_child_suspension(&mut child, output, pure_backend)
        {
            child.fiber.mark_trapped(FiberTrap {
                code: error.trap_code(),
                message: Some(error.to_string()),
                source_map: None,
            });
        }
        if child.fiber.status == FiberStatus::Running
            || (child.fiber.status == FiberStatus::Suspended && deferred.is_some())
        {
            let proof = match self.dialogues.inspect_commit(&transaction) {
                Ok(proof) => proof,
                Err(error) => {
                    if let Err(restore_error) = self.dialogues.restore_transaction(transaction) {
                        self.fail_with_error(restore_error.into(), output);
                        return true;
                    }
                    if !self.rollback_selected_child_step(
                        child,
                        checkpoint,
                        pending_host_call_before,
                        resume_journal,
                        need_states,
                        output,
                    ) {
                        return true;
                    }
                    self.fail_with_error(error.into(), output);
                    return true;
                }
            };
            resume_journal.commit_staged_need_ready(
                &mut self.need_producers,
                need_states,
                &self.program,
                &mut child.fiber,
            );
            resume_journal.commit_staged_host_call(&mut input.host_call_results, &mut child);
            let receipt = self.dialogues.commit_prepared(transaction, proof);
            self.child_fibers.push_back(child);
            resume_journal.commit(self, staged_resume_output, output);
            self.compact_pure_stats = candidate_stats;
            let commands = receipt.into_line().into_commands();
            output.requests.line_commands.extend(commands);
            self.consume_observations(observations, output);
            return true;
        }
        let failed = child.fiber.status == FiberStatus::Trapped;
        let cancelled = child.fiber.status == FiberStatus::Cancelled
            || phase == ProductLineTaskFiberPhase::Closing;
        let mut batch = ProductLineTaskExecutionBatch {
            child_fibers: VecDeque::new(),
            existing_child_actions: BTreeMap::new(),
            line_task_activations: Vec::new(),
            line_task_baseline: None,
            line_task_reserved_runs: Vec::new(),
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            observations,
            pure_stats: Some(candidate_stats),
        };
        if let Some(deferred_kind) = deferred.clone() {
            let failure = match child.fiber.terminal.as_ref() {
                Some(FiberTerminalValue::Trapped(trap)) => Some(trap.clone()),
                Some(FiberTerminalValue::Cancelled) => Some(FiberTrap {
                    code: AwbcTrapCode::InternalInvariant,
                    message: Some("defer child was cancelled".to_owned()),
                    source_map: None,
                }),
                Some(FiberTerminalValue::Returned(_)) => None,
                Some(FiberTerminalValue::DialogueResultSelected(_)) => Some(FiberTrap {
                    code: AwbcTrapCode::InternalInvariant,
                    message: Some("defer child selected a dialogue result".to_owned()),
                    source_map: None,
                }),
                None => Some(FiberTrap {
                    code: AwbcTrapCode::InternalInvariant,
                    message: Some("line-root defer child terminated without a value".to_owned()),
                    source_map: None,
                }),
            };
            match deferred_kind {
                ProductDeferredChildKind::LineRoot {
                    activation: deferred_activation,
                    registration,
                    site,
                } => {
                    if let Err(error) = transaction.line_mut().complete_deferred_child(
                        &deferred_activation,
                        registration,
                        site,
                        &after_handles,
                    ) {
                        if !self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            return true;
                        }
                        return self.begin_product_dialogue_failure(
                            transaction,
                            error.into(),
                            output,
                        );
                    }
                    if let Some(trap) = failure {
                        resume_journal.commit(self, staged_resume_output, output);
                        self.record_trap(&trap, output);
                        let (transaction, batch) =
                            match self.prepare_product_dialogue_failure(transaction, trap, batch) {
                                Ok(prepared) => prepared,
                                Err((transaction, batch, error)) => {
                                    self.child_fibers.push_front(child);
                                    self.fail_transaction_preflight_preserving_batch(
                                        transaction,
                                        batch,
                                        error,
                                        output,
                                    );
                                    return true;
                                }
                            };
                        return self.commit_product_dialogue_failure_close(
                            transaction,
                            batch,
                            output,
                        );
                    }
                }
                ProductDeferredChildKind::Scoped {
                    activation: deferred_activation,
                    frame: frame_instance,
                    scope: scope_id,
                    registration,
                    site,
                } => {
                    if let Err(error) = transaction.line_mut().complete_scoped_deferred_child(
                        &deferred_activation,
                        registration,
                        &after_handles,
                    ) {
                        if !self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            return true;
                        }
                        return self.begin_product_dialogue_failure(
                            transaction,
                            error.into(),
                            output,
                        );
                    }
                    let activation_fiber = match &mut transaction.frame_mut().phase {
                        ProductDialoguePhase::Activating { fiber, .. }
                        | ProductDialoguePhase::Closing(ProductDialogueClosing {
                            state: ProductDialogueClosingState::Activation { fiber, .. },
                            ..
                        }) => fiber,
                        _ => {
                            if !self.rollback_selected_child_step(
                                child,
                                checkpoint,
                                pending_host_call_before,
                                resume_journal,
                                need_states,
                                output,
                            ) {
                                return true;
                            }
                            return self.begin_product_dialogue_failure(
                                transaction,
                                ProductStepError::Line(
                                    crate::line_task::LineRuntimeError::InvalidDeferredTransition,
                                ),
                                output,
                            );
                        }
                    };
                    let active_frame = activation_fiber
                        .frames
                        .last_mut()
                        .filter(|frame| frame.instance == frame_instance);
                    let Some(active_frame) = active_frame else {
                        if !self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            return true;
                        }
                        return self.begin_product_dialogue_failure(
                            transaction,
                            ProductStepError::Line(
                                crate::line_task::LineRuntimeError::InvalidDeferredTransition,
                            ),
                            output,
                        );
                    };
                    let Some(scope) = active_frame
                        .scopes
                        .iter_mut()
                        .find(|scope| scope.id == scope_id)
                    else {
                        if !self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            return true;
                        }
                        return self.begin_product_dialogue_failure(
                            transaction,
                            ProductStepError::Line(
                                crate::line_task::LineRuntimeError::InvalidDeferredTransition,
                            ),
                            output,
                        );
                    };
                    if scope.defer_inflight
                        != Some(crate::awbc::fiber::FiberDeferredInFlight { registration, site })
                        || scope.defer_exit.is_none()
                    {
                        if !self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            return true;
                        }
                        return self.begin_product_dialogue_failure(
                            transaction,
                            ProductStepError::Line(
                                crate::line_task::LineRuntimeError::InvalidDeferredTransition,
                            ),
                            output,
                        );
                    }
                    scope.defer_inflight = None;
                    if scope.defer_failure.is_none() {
                        scope.defer_failure = failure;
                    }
                }
            }
            let prepared = match self.preflight_line_task_commands(&mut transaction, &batch) {
                Ok(prepared) => prepared,
                Err(error) => {
                    self.rollback_line_task_preview(&mut transaction, &mut batch);
                    if let Err(restore_error) = self.dialogues.restore_transaction(transaction) {
                        self.fail_with_error(restore_error.into(), output);
                        self.child_fibers.append(&mut batch.child_fibers);
                        return true;
                    }
                    if !self.rollback_selected_child_step(
                        child,
                        checkpoint,
                        pending_host_call_before,
                        resume_journal,
                        need_states,
                        output,
                    ) {
                        self.child_fibers.append(&mut batch.child_fibers);
                        return true;
                    }
                    self.child_fibers.append(&mut batch.child_fibers);
                    self.fail_with_error(error, output);
                    return true;
                }
            };
            let proof = match self.dialogues.inspect_commit(&transaction) {
                Ok(proof) => proof,
                Err(error) => {
                    self.rollback_line_task_preview(&mut transaction, &mut batch);
                    if let Err(restore_error) = self.dialogues.restore_transaction(transaction) {
                        self.fail_with_error(restore_error.into(), output);
                        self.child_fibers.append(&mut batch.child_fibers);
                    } else {
                        if self.rollback_selected_child_step(
                            child,
                            checkpoint,
                            pending_host_call_before,
                            resume_journal,
                            need_states,
                            output,
                        ) {
                            self.child_fibers.append(&mut batch.child_fibers);
                            self.fail_with_error(error.into(), output);
                        } else {
                            self.child_fibers.append(&mut batch.child_fibers);
                        }
                    }
                    return true;
                }
            };
            self.realize_line_task_commands(&mut transaction, &mut batch, prepared);
            let receipt = self.dialogues.commit_prepared(transaction, proof);
            resume_journal.commit(self, staged_resume_output, output);
            self.commit_line_task_commands(batch, output);
            output
                .requests
                .line_commands
                .extend(receipt.into_line().into_commands());
            return true;
        }
        let mut batch = match self.prepare_owned_line_task_completion(
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
                self.child_fibers.push_front(child);
                resume_journal.commit(self, staged_resume_output, output);
                return self.begin_product_dialogue_failure(transaction, error, output);
            }
        };
        if let Some(FiberTerminalValue::Trapped(trap)) = child.fiber.terminal.as_ref() {
            let trap = trap.clone();
            resume_journal.commit(self, staged_resume_output, output);
            self.record_trap(&trap, output);
            let (transaction, batch) =
                match self.prepare_product_dialogue_failure(transaction, trap, batch) {
                    Ok(prepared) => prepared,
                    Err((transaction, batch, error)) => {
                        self.child_fibers.push_front(child);
                        self.fail_transaction_preflight_preserving_batch(
                            transaction,
                            batch,
                            error,
                            output,
                        );
                        return true;
                    }
                };
            return self.commit_product_dialogue_failure_close(transaction, batch, output);
        }
        let prepared = match self.preflight_line_task_commands(&mut transaction, &batch) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.rollback_line_task_preview(&mut transaction, &mut batch);
                if let Err(restore_error) = self.dialogues.restore_transaction(transaction) {
                    self.child_fibers.push_front(child);
                    resume_journal.commit(self, staged_resume_output, output);
                    self.fail_with_error(restore_error.into(), output);
                    self.child_fibers.append(&mut batch.child_fibers);
                    return true;
                }
                self.child_fibers.push_front(child);
                resume_journal.commit(self, staged_resume_output, output);
                self.child_fibers.append(&mut batch.child_fibers);
                self.fail_with_error(error, output);
                return true;
            }
        };
        let proof = match self.dialogues.inspect_commit(&transaction) {
            Ok(proof) => proof,
            Err(error) => {
                self.rollback_line_task_preview(&mut transaction, &mut batch);
                if let Err(restore_error) = self.dialogues.restore_transaction(transaction) {
                    self.child_fibers.push_front(child);
                    resume_journal.commit(self, staged_resume_output, output);
                    self.fail_with_error(restore_error.into(), output);
                    self.child_fibers.append(&mut batch.child_fibers);
                } else {
                    self.child_fibers.push_front(child);
                    resume_journal.commit(self, staged_resume_output, output);
                    self.child_fibers.append(&mut batch.child_fibers);
                    self.fail_with_error(error.into(), output);
                }
                return true;
            }
        };
        self.realize_line_task_commands(&mut transaction, &mut batch, prepared);
        let receipt = self.dialogues.commit_prepared(transaction, proof);
        resume_journal.commit(self, staged_resume_output, output);
        self.commit_line_task_commands(batch, output);
        let commands = receipt.into_line().into_commands();
        output.requests.line_commands.extend(commands);
        true
    }

    fn initialize_suspension(
        &mut self,
        need_states: &mut Vec<RuntimeNeedState>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let dialogue = self
            .fiber
            .suspension
            .as_mut()
            .and_then(|suspension| match &mut suspension.reason {
                FiberSuspensionReason::Dialogue {
                    target,
                    target_type,
                    content,
                    values,
                    effects,
                    line_task_captures,
                    result,
                } => Some((
                    target.take(),
                    *target_type,
                    *content,
                    std::mem::take(values),
                    std::mem::take(effects),
                    std::mem::take(line_task_captures),
                    result.clone(),
                )),
                _ => None,
            });
        if let Some((target, target_type, content, values, effects, captures, result)) = dialogue {
            let Some(target) = target else {
                return;
            };
            self.present_dialogue(
                target,
                target_type,
                content,
                values,
                effects,
                captures,
                result,
                output,
            );
            return;
        }
        enum Dispatch {
            Choice(AwbcChoiceId),
            AwaitNeed(
                crate::task::TaskCorrelation,
                AwbcTypeId,
                Option<crate::awbc::schema::AwbcPatternId>,
                Option<crate::awbc::schema::AwbcAwaitObserverResume>,
            ),
            AwaitMany,
            HostCall(AwbcHostCallId, Vec<RuntimeValue>),
            InvalidHostCall,
            Other,
        }
        let Some((declared_resume, dispatch)) = self.fiber.suspension.as_ref().map(|suspension| {
            let dispatch = match &suspension.reason {
                FiberSuspensionReason::Dialogue { .. } => Dispatch::Other,
                FiberSuspensionReason::Choice { choice, .. } => Dispatch::Choice(*choice),
                FiberSuspensionReason::Await {
                    target:
                        FiberAwaitTarget::Need {
                            need, item_type, ..
                        },
                    binding,
                    observer,
                } => Dispatch::AwaitNeed(need.correlation(), *item_type, *binding, *observer),
                FiberSuspensionReason::AwaitMany(_) => Dispatch::AwaitMany,
                FiberSuspensionReason::HostCall { call, args, .. } => {
                    if args.iter().all(|value| value.ownership().permits_copy()) {
                        Dispatch::HostCall(*call, args.clone())
                    } else {
                        Dispatch::InvalidHostCall
                    }
                }
                FiberSuspensionReason::BudgetYield => Dispatch::Other,
            };
            (suspension.declared_resume(), dispatch)
        }) else {
            return;
        };
        match dispatch {
            Dispatch::Other => {}
            Dispatch::Choice(choice) => {
                self.present_choice(choice, output, pure_backend);
            }
            Dispatch::AwaitNeed(id, item_type, binding, observer) => {
                let task = self
                    .need_producers
                    .launch_for_correlation(&id)
                    .map(|launch| launch.task().clone());
                output.flow_events.push(FlowEvent::AwaitStarted {
                    need: id.need,
                    task,
                });
                if let Some(resume) = declared_resume {
                    self.resume_need(
                        &id,
                        item_type,
                        binding,
                        observer,
                        resume,
                        need_states,
                        output,
                    );
                }
            }
            Dispatch::AwaitMany => self.fill_await_many(output, pure_backend),
            Dispatch::HostCall(call, args) => self.emit_host_call(call, &args, output),
            Dispatch::InvalidHostCall => self.fail_with_trap(
                AwbcTrapCode::HostAbiMismatch,
                "external host-call arguments require deep Copy carriers".to_owned(),
                None,
                output,
            ),
        }
    }

    fn resume_main_suspension(
        &mut self,
        input: &mut RuntimeStepInput,
        need_states: &mut Vec<RuntimeNeedState>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        enum Dispatch {
            Choice(AwbcChoiceId, AwbcRegisterId),
            AwaitNeed(
                crate::task::TaskCorrelation,
                AwbcTypeId,
                Option<crate::awbc::schema::AwbcPatternId>,
                Option<crate::awbc::schema::AwbcAwaitObserverResume>,
            ),
            AwaitMany,
            HostCall(AwbcHostCallId, Option<AwbcRegisterId>),
            Other,
            BudgetYield,
        }
        let dialogue_resume = self.fiber.suspension.as_ref().and_then(|suspension| {
            matches!(&suspension.reason, FiberSuspensionReason::Dialogue { .. })
                .then(|| suspension.declared_resume())
        });
        let dialogue = self
            .fiber
            .suspension
            .as_mut()
            .and_then(|suspension| match &mut suspension.reason {
                FiberSuspensionReason::Dialogue {
                    target,
                    target_type,
                    content,
                    values,
                    effects,
                    line_task_captures,
                    result,
                } => Some((
                    dialogue_resume.flatten(),
                    target.take(),
                    *target_type,
                    *content,
                    std::mem::take(values),
                    std::mem::take(effects),
                    std::mem::take(line_task_captures),
                    result.clone(),
                )),
                _ => None,
            });
        if let Some((resume, target, target_type, content, values, effects, captures, result)) =
            dialogue
        {
            let Some(resume) = resume else {
                self.fail_with_error(
                    ProductStepError::Internal(
                        "non-budget suspension is missing a declared resume point".to_owned(),
                    ),
                    output,
                );
                return false;
            };
            return self.resume_dialogue(
                target,
                target_type,
                content,
                values,
                effects,
                captures,
                result,
                resume,
                input.dialogue_input_actions.as_slice(),
                &mut input.host_call_results,
                output,
                pure_backend,
            );
        }
        let Some(suspension) = self.fiber.suspension.as_ref() else {
            return false;
        };
        let declared_resume = suspension.declared_resume();
        let dispatch = match &suspension.reason {
            FiberSuspensionReason::Choice {
                choice,
                destination,
            } => Dispatch::Choice(*choice, *destination),
            FiberSuspensionReason::Await {
                target:
                    FiberAwaitTarget::Need {
                        need, item_type, ..
                    },
                binding,
                observer,
            } => Dispatch::AwaitNeed(need.correlation(), *item_type, *binding, *observer),
            FiberSuspensionReason::AwaitMany(_) => Dispatch::AwaitMany,
            FiberSuspensionReason::HostCall {
                call, destination, ..
            } => Dispatch::HostCall(*call, *destination),
            FiberSuspensionReason::Dialogue { .. } => Dispatch::Other,
            FiberSuspensionReason::BudgetYield => Dispatch::BudgetYield,
        };
        let Some(resume) = declared_resume else {
            if matches!(dispatch, Dispatch::BudgetYield | Dispatch::Other) {
                return false;
            }
            self.fail_with_error(
                ProductStepError::Internal(
                    "non-budget suspension is missing a declared resume point".to_owned(),
                ),
                output,
            );
            return false;
        };
        match dispatch {
            Dispatch::Other | Dispatch::BudgetYield => false,
            Dispatch::Choice(choice, destination) => {
                self.resume_choice(choice, destination, resume, input, output, pure_backend)
            }
            Dispatch::AwaitNeed(id, item_type, binding, observer) => self.resume_need(
                &id,
                item_type,
                binding,
                observer,
                resume,
                need_states,
                output,
            ),
            Dispatch::AwaitMany => {
                let before_handles = match line::product_fiber_handle_owners(
                    self.facade_fiber.execution,
                    &self.fiber,
                ) {
                    Ok(owners) => owners,
                    Err(error) => {
                        self.fail_with_error(error.into(), output);
                        return false;
                    }
                };
                let state = {
                    let Some(suspension) = self.fiber.suspension.as_mut() else {
                        return false;
                    };
                    match std::mem::replace(
                        &mut suspension.reason,
                        FiberSuspensionReason::BudgetYield,
                    ) {
                        FiberSuspensionReason::AwaitMany(state) => state,
                        reason => {
                            suspension.reason = reason;
                            return false;
                        }
                    }
                };
                self.resume_await_many(state, resume, before_handles, output, pure_backend)
            }
            Dispatch::HostCall(call, destination) => self.resume_host_call(
                call,
                destination,
                resume,
                &mut input.host_call_results,
                output,
            ),
        }
    }

    fn materialize_dialogue_effect_callbacks(
        &self,
        content: AwbcContentUnitId,
        effects: Box<[FiberDialogueContentEffectBinding]>,
    ) -> Result<
        BTreeMap<crate::runtime_id::RuntimeDialogueEffectSiteId, RuntimeCallableValue>,
        ProductStepError,
    > {
        let content_unit = self
            .program
            .content_units
            .get(content.index())
            .ok_or_else(|| ProductStepError::Input("dialogue content unit is absent".to_owned()))?;
        let template = self
            .program
            .content_templates
            .iter()
            .find(|template| template.id == content_unit.template)
            .ok_or_else(|| {
                ProductStepError::Input("dialogue content template is absent".to_owned())
            })?;
        if effects.len() != template.effects.len() {
            return Err(ProductStepError::Input(
                "dialogue effect callback rows disagree with the content template".to_owned(),
            ));
        }
        let callbacks = effects
            .into_vec()
            .into_iter()
            .enumerate()
            .map(|(index, binding)| {
                let expected_site =
                    crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                        .ok_or_else(|| {
                            ProductStepError::Input(
                                "dialogue effect site exceeds the runtime identity domain"
                                    .to_owned(),
                            )
                        })?;
                let declared = template.effects.get(index).ok_or_else(|| {
                    ProductStepError::Input("dialogue effect slot is absent".to_owned())
                })?;
                if binding.site != expected_site || binding.site != declared.site {
                    return Err(ProductStepError::Input(
                        "dialogue effect callback site is not canonical".to_owned(),
                    ));
                }
                let state = self
                    .program
                    .callable_states
                    .get(binding.state.index())
                    .ok_or(ProductStepError::Internal(
                        "dialogue effect callback state is absent".to_owned(),
                    ))?;
                if state.parameters.len() != 0
                    || !matches!(
                        state.attached,
                        crate::plan::RuntimeCallableAttachedContract::None
                    )
                    || state.retained.len() != declared.capture_types.len()
                    || binding.captures.len() != declared.capture_types.len()
                    || state
                        .retained
                        .iter()
                        .zip(&declared.capture_types)
                        .enumerate()
                        .any(|(position, (retained, expected))| {
                            retained.ty != *expected
                                || retained.role
                                    != (crate::plan::RuntimeCallableRetainedRole::Capture {
                                        position: u32::try_from(position).unwrap_or(u32::MAX),
                                    })
                        })
                {
                    return Err(ProductStepError::Input(
                        "dialogue effect callback ABI disagrees with its manifest".to_owned(),
                    ));
                }
                if binding
                    .captures
                    .iter()
                    .zip(&declared.capture_types)
                    .any(|(value, expected)| {
                        !runtime_value_matches_type(&self.program, value, *expected, 0)
                    })
                {
                    return Err(ProductStepError::Type(
                        "dialogue effect capture value has the wrong runtime type".to_owned(),
                    ));
                }
                let callback = RuntimeCallableValue::try_new(
                    crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
                    binding.state,
                    binding.captures.into_vec(),
                )
                .map_err(|error| ProductStepError::Internal(error.to_string()))?;
                crate::awbc::fiber::validate_runtime_callable_activation(&self.program, &callback)
                    .map_err(|error| ProductStepError::Internal(error.to_string()))?;
                Ok((binding.site, callback))
            })
            .collect::<Result<Vec<_>, ProductStepError>>()?;
        Ok(callbacks.into_iter().collect())
    }

    fn present_dialogue(
        &mut self,
        target: crate::value::RuntimeOpaqueValue,
        target_type: crate::awbc::schema::AwbcTypeId,
        content: AwbcContentUnitId,
        values: Box<[crate::plan::RuntimeDialogueValueBinding]>,
        effects: Box<[FiberDialogueContentEffectBinding]>,
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
        let effect_callbacks = match self.materialize_dialogue_effect_callbacks(content, effects) {
            Ok(callbacks) => callbacks,
            Err(error) => {
                self.record_error(error, output);
                return;
            }
        };
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
            crate::awbc::fiber::AwbcFiberRoot::Function(group.activation),
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
            target,
            target_type,
            line: line_id,
            captures,
            task_inputs: Box::default(),
            values,
            effect_callbacks,
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
            pending_activation_host_call: None,
        };
        if let Err(error) = self.dialogues.begin(active) {
            self.fail_with_error(error.into(), output);
            return;
        }
        self.dialogue_occurrences
            .insert(occurrence_key, next_occurrence);
        self.next_generation = next_generation;
        self.next_fiber_instance = next_fiber_instance;
        self.fiber.line_cursor = next_line_cursor;
    }

    fn resume_dialogue(
        &mut self,
        target: Option<crate::value::RuntimeOpaqueValue>,
        target_type: crate::awbc::schema::AwbcTypeId,
        content: AwbcContentUnitId,
        values: Box<[crate::plan::RuntimeDialogueValueBinding]>,
        effects: Box<[FiberDialogueContentEffectBinding]>,
        captures: Box<[RuntimeValue]>,
        result: crate::awbc::schema::AwbcDialogueResultTarget,
        resume: AwbcResumePointId,
        input_actions: &[RuntimeDialogueInputActionEvent],
        host_call_results: &mut Vec<crate::step::RuntimeHostCallResult>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        if self.dialogues.active_frame().is_none() {
            let Some(target) = target else {
                self.fail_with_error(
                    ProductStepError::Internal(
                        "suspended dialogue lost its target before Product admission".to_owned(),
                    ),
                    output,
                );
                return false;
            };
            self.present_dialogue(
                target,
                target_type,
                content,
                values,
                effects,
                captures,
                result,
                output,
            );
        }
        let Ok(mut transaction) = self.dialogues.begin_active_transaction() else {
            return false;
        };
        let activation = transaction.activation().clone();
        if matches!(transaction.frame().phase, ProductDialoguePhase::Closing(_)) {
            return self.resume_product_dialogue_failure_close(transaction, output);
        }
        if matches!(
            transaction.frame().phase,
            ProductDialoguePhase::Activating { .. }
        ) {
            let progress = match self.step_dialogue_activation_with_host_results(
                &mut transaction,
                host_call_results,
                pure_backend,
            ) {
                Ok(progress) => progress,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
            let candidate_pure_stats = progress.pure_stats;
            let host_result_take = progress.host_result_take;
            let command_batch_result = match progress.execution {
                Some(batch) => Ok(batch),
                None => self.prepare_line_task_commands(&mut transaction, progress.reducer),
            };
            let command_batch = match command_batch_result {
                Ok(batch) => batch,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
            let mut command_batch = command_batch;
            let prepared = match self.preflight_line_task_commands(&mut transaction, &command_batch)
            {
                Ok(prepared) => prepared,
                Err(error) => {
                    return self.begin_product_dialogue_failure_with_batch(
                        transaction,
                        error,
                        command_batch,
                        output,
                    );
                }
            };
            let proof = match self.dialogues.inspect_commit(&transaction) {
                Ok(proof) => proof,
                Err(error) => {
                    self.rollback_line_task_preview(&mut transaction, &mut command_batch);
                    if let Err(restore_error) = self.dialogues.restore_transaction(transaction) {
                        self.child_fibers.append(&mut command_batch.child_fibers);
                        self.fail_with_error(restore_error.into(), output);
                    } else {
                        self.child_fibers.append(&mut command_batch.child_fibers);
                        self.fail_with_error(error.into(), output);
                    }
                    return true;
                }
            };
            if let Some(ticket) = host_result_take {
                self.commit_activation_host_result(&mut transaction, host_call_results, ticket);
            }
            self.realize_line_task_commands(&mut transaction, &mut command_batch, prepared);
            let receipt = self.dialogues.commit_prepared(transaction, proof);
            if let Some(stats) = candidate_pure_stats {
                self.compact_pure_stats = stats;
            }
            self.commit_line_task_commands(command_batch, output);
            output.requests.host_calls.extend(progress.host_calls);
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
        let (content, content_events, advance) = {
            let active = transaction.frame_mut();
            (
                active.content,
                std::mem::take(&mut active.pending_content_events),
                std::mem::take(&mut active.pending_advance),
            )
        };
        let Some(content_unit) = self.program.content_units.get(content.index()).cloned() else {
            return self.begin_product_dialogue_failure(
                transaction,
                ProductStepError::Internal(
                    "active dialogue references an absent AWBC content unit".to_owned(),
                ),
                output,
            );
        };
        let accepted_content = match transaction.frame_mut().line_task_mut() {
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
        let callback_sites = content_events
            .iter()
            .filter_map(|event| match event {
                crate::step::RuntimeDialogueContentEventKind::Mark(_) => None,
                crate::step::RuntimeDialogueContentEventKind::Effect(site) => Some(*site),
            })
            .collect::<Vec<_>>();
        let accepted_input_actions = match transaction.frame_mut().line_task_mut() {
            Some(line_task) => match line_task.accept_input_action_events(input_actions) {
                Ok(actions) => actions,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error.into(), output);
                }
            },
            None => Vec::new(),
        };
        let ready = accepted_content
            .ready()
            .with_input_actions(&accepted_input_actions);
        let mut reducer_activation =
            match self.progress_line_task(transaction.frame_mut(), &accepted_content) {
                Ok(activation) => activation,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
        let (cancel_trigger, cancel_activation) =
            match self.cancel_line_task(transaction.frame_mut(), ready) {
                Ok(result) => result,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
        reducer_activation.append(cancel_activation);
        if advance {
            let finish_activation = match self.finish_line_task(transaction.frame_mut()) {
                Ok(activation) => activation,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
            reducer_activation.append(finish_activation);
        }
        let mut command_batch =
            match self.prepare_line_task_commands(&mut transaction, reducer_activation) {
                Ok(batch) => batch,
                Err(error) => {
                    return self.begin_product_dialogue_failure(transaction, error, output);
                }
            };
        if let Err(error) = self.stage_dialogue_effect_callbacks(
            &mut command_batch,
            &mut transaction,
            &callback_sites,
        ) {
            return self.begin_product_dialogue_failure_with_batch(
                transaction,
                error,
                command_batch,
                output,
            );
        }
        if transaction
            .frame()
            .line_task()
            .is_some_and(LineTaskLiveState::is_closed)
        {
            let exit = self
                .closed_line_task_exit(transaction.frame())
                .ok_or_else(|| {
                    ProductStepError::Internal(
                        "closed line-task frame has no fixed scope exit".to_owned(),
                    )
                });
            let exit = match exit {
                Ok(exit) => exit,
                Err(error) => {
                    return self.begin_product_dialogue_failure_with_batch(
                        transaction,
                        error,
                        command_batch,
                        output,
                    );
                }
            };
            if let Err(error) =
                self.prepare_next_deferred_child(&mut transaction, exit, &mut command_batch)
            {
                return self.begin_product_dialogue_failure_with_batch(
                    transaction,
                    error,
                    command_batch,
                    output,
                );
            }
            if transaction.line().deferred_inflight().is_some() {
                let Some((registration, site)) = transaction.line().deferred_inflight() else {
                    unreachable!("checked in-flight deferred registration")
                };
                let child_is_present = command_batch
                    .child_fibers
                    .iter()
                    .chain(&self.child_fibers)
                    .any(|child| {
                        matches!(
                            &child.owner,
                            ProductChildFiberOwner::Deferred {
                                activation: child_activation,
                                registration: child_registration,
                                site: child_site,
                                ..
                            } if child_activation == &activation
                                && *child_registration == registration
                                && *child_site == site
                        )
                    });
                if !child_is_present {
                    return self.begin_product_dialogue_failure_with_batch(
                        transaction,
                        ProductStepError::Internal(
                            "inflight line-root defer has no executor child".to_owned(),
                        ),
                        command_batch,
                        output,
                    );
                }
            }
            if transaction.line().deferred_inflight().is_some()
                || !transaction.line().deferred_registrations().is_empty()
            {
                let progressed = transaction.line().deferred_inflight().is_none();
                let prepared =
                    match self.preflight_line_task_commands(&mut transaction, &command_batch) {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            return self.begin_product_dialogue_failure_with_batch(
                                transaction,
                                error,
                                command_batch,
                                output,
                            );
                        }
                    };
                let proof = match self.dialogues.inspect_commit(&transaction) {
                    Ok(proof) => proof,
                    Err(error) => {
                        self.rollback_line_task_preview(&mut transaction, &mut command_batch);
                        if let Err(restore_error) = self.dialogues.restore_transaction(transaction)
                        {
                            self.child_fibers.append(&mut command_batch.child_fibers);
                            self.fail_with_error(restore_error.into(), output);
                        } else {
                            self.child_fibers.append(&mut command_batch.child_fibers);
                            self.fail_with_error(error.into(), output);
                        }
                        return true;
                    }
                };
                self.realize_line_task_commands(&mut transaction, &mut command_batch, prepared);
                let receipt = self.dialogues.commit_prepared(transaction, proof);
                self.commit_line_task_commands(command_batch, output);
                output
                    .requests
                    .line_commands
                    .extend(receipt.into_line().into_commands());
                return progressed;
            }
            let publication = match self.prepare_dialogue_publication(&mut transaction, resume) {
                Ok(publication) => publication,
                Err(error) => {
                    return self.begin_product_dialogue_failure_with_batch(
                        transaction,
                        error,
                        command_batch,
                        output,
                    );
                }
            };
            return match publication {
                line::ProductPublicationProgress::Pending => {
                    let prepared =
                        match self.preflight_line_task_commands(&mut transaction, &command_batch) {
                            Ok(prepared) => prepared,
                            Err(error) => {
                                return self.begin_product_dialogue_failure_with_batch(
                                    transaction,
                                    error,
                                    command_batch,
                                    output,
                                );
                            }
                        };
                    let proof = match self.dialogues.inspect_commit(&transaction) {
                        Ok(proof) => proof,
                        Err(error) => {
                            self.rollback_line_task_preview(&mut transaction, &mut command_batch);
                            if let Err(restore_error) =
                                self.dialogues.restore_transaction(transaction)
                            {
                                self.child_fibers.append(&mut command_batch.child_fibers);
                                self.fail_with_error(restore_error.into(), output);
                            } else {
                                self.child_fibers.append(&mut command_batch.child_fibers);
                                self.fail_with_error(error.into(), output);
                            }
                            return false;
                        }
                    };
                    self.realize_line_task_commands(&mut transaction, &mut command_batch, prepared);
                    let receipt = self.dialogues.commit_prepared(transaction, proof);
                    let commands = receipt.into_line().into_commands();
                    self.commit_line_task_commands(command_batch, output);
                    output.requests.line_commands.extend(commands);
                    if let Some(trigger) = cancel_trigger {
                        output.flow_events.push(FlowEvent::LineCancelled {
                            trigger: trigger.as_str().to_owned(),
                        });
                    }
                    false
                }
                line::ProductPublicationProgress::Ready {
                    resume: prepared_resume,
                    pattern: prepared_pattern,
                } => {
                    let prepared =
                        match self.preflight_line_task_commands(&mut transaction, &command_batch) {
                            Ok(prepared) => prepared,
                            Err(error) => {
                                return self.begin_product_dialogue_failure_with_batch(
                                    transaction,
                                    error,
                                    command_batch,
                                    output,
                                );
                            }
                        };
                    let proof = match self.dialogues.inspect_published(&transaction) {
                        Ok(proof) => proof,
                        Err(error) => {
                            self.rollback_line_task_preview(&mut transaction, &mut command_batch);
                            if let Err(restore_error) =
                                self.dialogues.restore_transaction(transaction)
                            {
                                self.child_fibers.append(&mut command_batch.child_fibers);
                                self.fail_with_error(restore_error.into(), output);
                            } else {
                                self.child_fibers.append(&mut command_batch.child_fibers);
                                self.fail_with_error(error.into(), output);
                            }
                            return false;
                        }
                    };
                    self.realize_line_task_commands(&mut transaction, &mut command_batch, prepared);
                    transaction
                        .line_mut()
                        .release_frame()
                        .expect("published dialogue release was preflighted");
                    let (_, value) = transaction
                        .line_mut()
                        .finish_result_publication()
                        .expect("published result owner was preflighted");
                    crate::awbc::vm::bind_pattern_owned_prepared(
                        &self.program,
                        &mut self.fiber,
                        prepared_pattern,
                        value,
                    );
                    self.fiber.resume_at_prepared(prepared_resume);
                    let receipt = self.dialogues.commit_published_prepared(transaction, proof);
                    let commands = receipt.into_line().into_commands();
                    self.commit_line_task_commands(command_batch, output);
                    output.requests.line_commands.extend(commands);
                    if let Some(trigger) = cancel_trigger {
                        output.flow_events.push(FlowEvent::LineCancelled {
                            trigger: trigger.as_str().to_owned(),
                        });
                    }
                    true
                }
            };
        }
        let prepared = match self.preflight_line_task_commands(&mut transaction, &command_batch) {
            Ok(prepared) => prepared,
            Err(error) => {
                return self.begin_product_dialogue_failure_with_batch(
                    transaction,
                    error,
                    command_batch,
                    output,
                );
            }
        };
        let proof = match self.dialogues.inspect_commit(&transaction) {
            Ok(proof) => proof,
            Err(error) => {
                self.rollback_line_task_preview(&mut transaction, &mut command_batch);
                if let Err(restore_error) = self.dialogues.restore_transaction(transaction) {
                    self.child_fibers.append(&mut command_batch.child_fibers);
                    self.fail_with_error(restore_error.into(), output);
                } else {
                    self.child_fibers.append(&mut command_batch.child_fibers);
                    self.fail_with_error(error.into(), output);
                }
                return false;
            }
        };
        self.realize_line_task_commands(&mut transaction, &mut command_batch, prepared);
        let receipt = self.dialogues.commit_prepared(transaction, proof);
        self.commit_line_task_commands(command_batch, output);
        let commands = receipt.into_line().into_commands();
        output.requests.line_commands.extend(commands);
        if let Some(trigger) = cancel_trigger {
            output.flow_events.push(FlowEvent::LineCancelled {
                trigger: trigger.as_str().to_owned(),
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
        let batch = ProductLineTaskExecutionBatch {
            child_fibers: VecDeque::new(),
            existing_child_actions: BTreeMap::new(),
            line_task_activations: Vec::new(),
            line_task_baseline: None,
            line_task_reserved_runs: Vec::new(),
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            observations: Vec::new(),
            pure_stats: None,
        };
        self.begin_product_dialogue_failure_with_batch(transaction, error, batch, output)
    }

    fn begin_product_dialogue_failure_with_batch(
        &mut self,
        mut transaction: dialogue::ProductDialogueTransaction,
        error: ProductStepError,
        mut batch: ProductLineTaskExecutionBatch,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        self.rollback_line_task_preview(&mut transaction, &mut batch);
        let message = error.to_string();
        let trap = FiberTrap {
            code: error.trap_code(),
            message: Some(message),
            source_map: None,
        };
        self.record_error(error, output);
        let (transaction, batch) =
            match self.prepare_product_dialogue_failure(transaction, trap, batch) {
                Ok(prepared) => prepared,
                Err((transaction, batch, cleanup)) => {
                    self.fail_transaction_preflight_preserving_batch(
                        transaction,
                        batch,
                        cleanup,
                        output,
                    );
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
        transaction: dialogue::ProductDialogueTransaction,
        trap: FiberTrap,
        mut batch: ProductLineTaskExecutionBatch,
    ) -> Result<
        (
            dialogue::ProductDialogueTransaction,
            ProductLineTaskExecutionBatch,
        ),
        (
            dialogue::ProductDialogueTransaction,
            ProductLineTaskExecutionBatch,
            ProductStepError,
        ),
    > {
        if matches!(transaction.frame().phase, ProductDialoguePhase::Closing(_)) {
            return Ok((transaction, batch));
        }
        let reducing_view = if matches!(
            transaction.frame().phase,
            ProductDialoguePhase::Reducing { .. }
        ) {
            match self.line_task_view(transaction.frame().content) {
                Some(view) => Some(view),
                None => {
                    return Err((
                        transaction,
                        batch,
                        LineRuntimeError::UnknownTaskGroup.into(),
                    ));
                }
            }
        } else {
            None
        };
        let mut reducer = crate::line_task::LineTaskActivation::default();
        let mut transaction = transaction.map_frame(|mut frame| {
            let prior = frame.phase;
            let closing_state = match prior {
                ProductDialoguePhase::Activating { fiber, pending } => {
                    ProductDialogueClosingState::Activation { fiber, pending }
                }
                ProductDialoguePhase::Reducing { mut line_task } => {
                    let view = reducing_view.expect("reducing task view was preflighted");
                    reducer = fail_live_line_task_group(&view, &mut line_task);
                    ProductDialogueClosingState::LineTask { line_task }
                }
                ProductDialoguePhase::Publishing { line_task } => {
                    ProductDialogueClosingState::LineTask { line_task }
                }
                ProductDialoguePhase::Closing(_) => unreachable!("closing phase was handled"),
                ProductDialoguePhase::Transitioning => {
                    unreachable!("transitioning phase cannot enter failure close")
                }
            };
            frame.phase = ProductDialoguePhase::Closing(ProductDialogueClosing {
                failure: trap,
                state: closing_state,
            });
            frame.pending_content_events.clear();
            frame.pending_advance = false;
            frame
        });
        if let Err(error) = transaction.line_mut().abandon() {
            return Err((transaction, batch, error.into()));
        }
        if let Err(error) =
            self.prepare_line_task_commands_from(&mut transaction, reducer, &mut batch)
        {
            return Err((transaction, batch, error));
        }
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
        line::settle_scoped_defer_releases(&mut transaction);
        let mut batch = ProductLineTaskExecutionBatch {
            child_fibers: VecDeque::new(),
            existing_child_actions: BTreeMap::new(),
            line_task_activations: Vec::new(),
            line_task_baseline: None,
            line_task_reserved_runs: Vec::new(),
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            observations: Vec::new(),
            pure_stats: None,
        };
        let (scope_progress, scope_failure) =
            match self.unwind_failed_activation_scope(&mut transaction, &mut batch) {
                Ok(result) => result,
                Err(cleanup) => {
                    output.diagnostics.push(RuntimeDiagnostic::new(format!(
                        "dialogue cleanup after primary failure also failed: {cleanup}"
                    )));
                    self.fail_transaction_preflight_preserving_batch(
                        transaction,
                        batch,
                        cleanup,
                        output,
                    );
                    return true;
                }
            };
        if let Some(trap) = scope_failure {
            output.diagnostics.push(RuntimeDiagnostic::new(format!(
                "dialogue cleanup after primary failure also failed: {trap:?}"
            )));
        }
        if scope_progress {
            let prepared = match self.preflight_line_task_commands(&mut transaction, &batch) {
                Ok(prepared) => prepared,
                Err(error) => {
                    self.fail_transaction_preflight_preserving_batch(
                        transaction,
                        batch,
                        error,
                        output,
                    );
                    return true;
                }
            };
            let proof = match self.dialogues.inspect_commit(&transaction) {
                Ok(proof) => proof,
                Err(error) => {
                    self.fail_transaction_preflight_preserving_batch(
                        transaction,
                        batch,
                        error.into(),
                        output,
                    );
                    return true;
                }
            };
            self.realize_line_task_commands(&mut transaction, &mut batch, prepared);
            let receipt = self.dialogues.commit_prepared(transaction, proof);
            self.commit_line_task_commands(batch, output);
            output
                .requests
                .line_commands
                .extend(receipt.into_line().into_commands());
            return true;
        }
        let reducer_closed = match &transaction.frame().phase {
            ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::Activation { .. },
                ..
            }) => true,
            ProductDialoguePhase::Closing(ProductDialogueClosing {
                state: ProductDialogueClosingState::LineTask { line_task },
                ..
            }) => line_task.is_closed(),
            ProductDialoguePhase::Activating { .. }
            | ProductDialoguePhase::Reducing { .. }
            | ProductDialoguePhase::Publishing { .. }
            | ProductDialoguePhase::Transitioning => false,
        };
        if reducer_closed {
            let exit = transaction
                .line()
                .deferred_exit()
                .unwrap_or(ScopeExit::Failed);
            let mut batch = batch;
            let progressed =
                match self.prepare_next_deferred_child(&mut transaction, exit, &mut batch) {
                    Ok(progressed) => progressed,
                    Err(cleanup) => {
                        output.diagnostics.push(RuntimeDiagnostic::new(format!(
                            "dialogue cleanup after primary failure also failed: {cleanup}"
                        )));
                        self.fail_with_error(cleanup, output);
                        return true;
                    }
                };
            if transaction.line().deferred_inflight().is_some()
                || !transaction.line().deferred_registrations().is_empty()
            {
                let waiting_for_child = transaction.line().deferred_inflight().is_some();
                let completed =
                    self.commit_product_dialogue_failure_close(transaction, batch, output);
                return completed || (!waiting_for_child && progressed);
            }
            if let Err(cleanup) = transaction
                .line_mut()
                .prepare_handle_unwind(&activation, false)
            {
                output.diagnostics.push(RuntimeDiagnostic::new(format!(
                    "dialogue cleanup after primary failure also failed: {cleanup}"
                )));
            }
            return self.commit_product_dialogue_failure_close(transaction, batch, output);
        }
        let batch = batch;
        self.commit_product_dialogue_failure_close(transaction, batch, output)
    }

    fn commit_product_dialogue_failure_close(
        &mut self,
        mut transaction: dialogue::ProductDialogueTransaction,
        mut batch: ProductLineTaskExecutionBatch,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let prepared = match self.preflight_line_task_commands(&mut transaction, &batch) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_transaction_preflight_preserving_batch(transaction, batch, error, output);
                return true;
            }
        };
        let activation = transaction.activation().clone();
        let terminal = transaction.line().failure_close_ready()
            && !batch.has_joined_dialogue_work(&activation, &self.child_fibers, &prepared);
        if terminal {
            let proof = match self.dialogues.inspect_abandoned(&transaction) {
                Ok(proof) => proof,
                Err(error) => {
                    self.fail_transaction_preflight_preserving_batch(
                        transaction,
                        batch,
                        error.into(),
                        output,
                    );
                    return true;
                }
            };
            if let Err(error) = transaction.line_mut().release_frame() {
                self.fail_transaction_preflight_preserving_batch(
                    transaction,
                    batch,
                    error.into(),
                    output,
                );
                return true;
            }
            let failure = match &transaction.frame().phase {
                ProductDialoguePhase::Closing(closing) => closing.failure.clone(),
                ProductDialoguePhase::Activating { .. }
                | ProductDialoguePhase::Reducing { .. }
                | ProductDialoguePhase::Publishing { .. }
                | ProductDialoguePhase::Transitioning => {
                    self.fail_with_error(LineRuntimeError::InvalidResultTransition.into(), output);
                    return true;
                }
            };
            self.realize_line_task_commands(&mut transaction, &mut batch, prepared);
            let receipt = self.dialogues.commit_abandoned_prepared(transaction, proof);
            self.commit_line_task_commands(batch, output);
            let commands = receipt.into_line().into_commands();
            output.requests.line_commands.extend(commands);
            self.terminate_with_trap(failure, output);
            true
        } else {
            let proof = match self.dialogues.inspect_commit(&transaction) {
                Ok(proof) => proof,
                Err(error) => {
                    self.fail_transaction_preflight_preserving_batch(
                        transaction,
                        batch,
                        error.into(),
                        output,
                    );
                    return true;
                }
            };
            self.realize_line_task_commands(&mut transaction, &mut batch, prepared);
            let receipt = self.dialogues.commit_prepared(transaction, proof);
            self.commit_line_task_commands(batch, output);
            let commands = receipt.into_line().into_commands();
            output.requests.line_commands.extend(commands);
            false
        }
    }

    fn fail_transaction_preflight_preserving_batch(
        &mut self,
        mut transaction: dialogue::ProductDialogueTransaction,
        mut batch: ProductLineTaskExecutionBatch,
        error: ProductStepError,
        output: &mut RuntimeStepOutput,
    ) {
        self.rollback_line_task_preview(&mut transaction, &mut batch);
        if let Err(restore_error) = self.dialogues.restore_transaction(transaction) {
            self.child_fibers.append(&mut batch.child_fibers);
            self.fail_with_error(restore_error.into(), output);
        } else {
            self.child_fibers.append(&mut batch.child_fibers);
            self.fail_with_error(error, output);
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
        events: crate::line_task::LineTaskReadyEvents<'_>,
    ) -> Result<
        (
            Option<arcweft_interaction_model::input::InputActionId>,
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
            match cancel_live_line_task_group(&view, events, state) {
                Some((action, activation)) => (Some(action), activation),
                None => (None, crate::line_task::LineTaskActivation::default()),
            }
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

    fn closed_line_task_exit(&self, active: &ActiveDialogue) -> Option<ScopeExit> {
        let line_task = active.line_task()?;
        match line_task.snapshot().phase() {
            crate::line_task::LineTaskPhase::Closed { exit } => Some(exit),
            crate::line_task::LineTaskPhase::Active
            | crate::line_task::LineTaskPhase::Closing { .. } => None,
        }
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
        mut batch: ProductLineTaskExecutionBatch,
    ) -> Result<ProductLineTaskExecutionBatch, ProductStepError> {
        if transaction.frame().content != content {
            return Err(ProductStepError::StaleLineTaskChildContent {
                expected: transaction.frame().content,
                actual: content,
            });
        }
        let (selected_tokens, selected_live) = if let Some(
            FiberTerminalValue::DialogueResultSelected(value),
        ) = child.terminal.as_ref()
        {
            if !joined {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            let group_id = self
                .dialogue_group(content)
                .ok_or(LineRuntimeError::MissingTaskGroup)?;
            let group = self
                .program
                .line_task_groups
                .get(group_id.index())
                .ok_or(LineRuntimeError::UnknownTaskGroup)?;
            if transaction.frame().result.ty != group.result_type
                || !runtime_value_matches_type(&self.program, value, group.result_type, 0)
            {
                return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
            }
            let selected_tokens = line::unique_line_handles(value)?
                .into_iter()
                .map(|handle| handle.token().clone())
                .collect::<BTreeSet<_>>();
            let selected_live =
                line::product_fiber_handle_tokens(self.facade_fiber.execution, child)?;
            if selected_tokens
                .iter()
                .any(|token| !selected_live.contains(token))
            {
                return Err(LineRuntimeError::WrongOwner.into());
            }
            let view = self
                .line_task_view(content)
                .ok_or(LineRuntimeError::UnknownTaskGroup)?;
            let reducer = transaction
                .frame_mut()
                .line_task_mut()
                .ok_or(LineRuntimeError::InvalidActivationOperation)?;
            if !reducer.accepts_result_selection(&view, &tag) {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            let prepared = transaction.line_mut().validate_result_selection(
                &tag,
                &group.result_type,
                value,
            )?;
            let Some(FiberTerminalValue::DialogueResultSelected(value)) = child.terminal.take()
            else {
                unreachable!("borrowed result selection was preflighted")
            };
            transaction
                .line_mut()
                .select_result_prepared(prepared, value);
            (selected_tokens, Some(selected_live))
        } else {
            (BTreeSet::new(), None)
        };
        if let Some(token) = tag.scheduled_token().cloned() {
            let live = if let Some(live) = selected_live.as_ref() {
                live.clone()
            } else {
                line::product_fiber_handle_tokens(self.facade_fiber.execution, child)?
            };
            let locals = transaction.line().scheduled_child_locals(&token)?;
            let values = child.take_function_argument_storage(&self.program)?;
            if values.len() != locals.len() {
                return Err(
                    crate::line_task::LineRuntimeError::InvalidScheduledCaptureGraph.into(),
                );
            }
            let mut returned_bindings = Vec::new();
            let mut returned = BTreeSet::new();
            for (local, value) in locals.into_vec().into_iter().zip(values) {
                let handles = value
                    .values()
                    .map(line::unique_line_handles)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                if handles
                    .iter()
                    .any(|handle| selected_tokens.contains(handle.token()))
                {
                    // A selected affine result has moved to DialogueResult.
                    // Do not return another binding that still names the same
                    // token; any other handles in this discarded aggregate
                    // remain in `live` and are released by finish_child_scope.
                    continue;
                }
                returned.extend(handles.into_iter().map(|handle| handle.token().clone()));
                returned_bindings.push(crate::value::RuntimeLocalSlot::new(local, value));
            }
            let returned_bindings = returned_bindings.into_boxed_slice();
            let live = live
                .difference(&selected_tokens)
                .cloned()
                .collect::<BTreeSet<_>>();
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
        self.prepare_line_task_commands_from(transaction, completion, &mut batch)?;
        Ok(batch)
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
                    Vec::new(),
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
