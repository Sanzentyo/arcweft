use crate::awbc::schema::AwbcTypeId;
use crate::effect::LineEffectRequest;
use crate::entry::{RuntimeCallableExecutableCode, RuntimeCallableRole};
use crate::line_task::{ChildCancelPolicy, ChildJoinPolicy, LineTaskGroup, LineTaskWorkTag};
use crate::observation::RuntimeObservationState;
use crate::pattern::RuntimePattern;
use crate::plan::{
    ChoiceRuntimeOption, EntryRuntimeId, FlowEvent, FlowOp, FlowRuntimeId, RuntimeEntryTarget,
    RuntimeFlow, RuntimeFunctionInputSource, RuntimeFunctionSiteBody, RuntimePlan,
};
use crate::pure::{RuntimeCallBackend, VmPureFunctionScratch, VmRuntimePureCallBackend};
use crate::root::{
    RootCallableEvaluationError, RootCallableEvaluator, RootEventInput, RootRuntime,
    RootRuntimeError, RootStartupContract, RuntimeCommandEnvelope,
};
use crate::runtime_id::{DialogueActivationId, RuntimePersistentFiberId};
use crate::runtime_id::{RuntimeFormatAttemptId, RuntimePlanTypeId};
use crate::step::{
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeStepInput,
    RuntimeStepMode, RuntimeStepOptions, RuntimeStepOutput, RuntimeStepResult, RuntimeStepStats,
    RuntimeStepStopReason,
};
use crate::stream::{
    RuntimeStreamEvent, StreamMatchArm, StreamOp, StreamRuntimeId, StreamRuntimeState,
};
use crate::task::{
    AwaitManyTarget, CancelScopeId, GenerationId, NeedId, NeedProducerOwnedTaskEventDisposition,
    NeedProducerRegistry, RuntimeNeedPublication, TaskEvent, TaskEventKind, TaskId, TaskKey,
    TaskPolicy, TaskPriority, TaskPublicationCursor, TaskSpec, normalize_runtime_need_states,
    normalize_task_events,
};
use crate::value::{
    RuntimeCallableValue, RuntimeCallableZeroArgInvocationProof,
    RuntimeDialogueContentEffectBinding, RuntimeEnv, RuntimeEvalError, RuntimeExpr,
    RuntimeExprMatchArm, RuntimeFlowParameterBinding, RuntimeIterator, RuntimeLocalBinding,
    RuntimePayload, RuntimeSeq, RuntimeValue, evaluate_binary, evaluate_unary,
    runtime_sequence_dense_i64, runtime_sequence_from_literal_values,
    runtime_sequence_repeat_value, runtime_sequence_values, runtime_value_into_sequence_values,
    runtime_value_label, sum_i64_sequence_ref,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use thiserror::Error;
pub mod aot;
pub mod audio;
pub mod dialogue;
pub mod eval;
mod numeric_map;
pub(crate) use eval::evaluate_runtime_call;
pub mod flow;
pub mod line;
pub mod program;
pub mod stream;
pub mod suspend;

/// Ephemeral charge for the currently dispatched native operation. The driver
/// reserves that dispatch first; physical bodies may consume only its actual
/// remaining step budget. It is cleared before the next operation/host return.
#[derive(Clone, Copy, Debug, PartialEq)]
struct NativeScalarStepCharge {
    mode: RuntimeStepMode,
    fiber: FlowFiberId,
    remaining: usize,
    charged: usize,
}
/// One native root result authority. Function roots are internal deterministic
/// invocations of the exact admitted site; program roots retain their public
/// program binding. Both use the same frame, transaction and value custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeInvocationRoot {
    Program(arcweft_id::runtime_program::RuntimePureProgramId),
    Function(crate::runtime_id::RuntimeFunctionSiteId),
}

#[derive(Debug, PartialEq)]
pub struct Engine {
    evaluation_stats: crate::pure::PureFunctionStats,
    invocation_result: Option<(NativeInvocationRoot, RuntimeValue)>,
    plan: Arc<RuntimePlan>,
    format_context: crate::value::RuntimeFormatContext,
    generation: GenerationId,
    need_producers: NeedProducerRegistry,
    task_request_quota_remaining: usize,
    scalar_step_charge: Option<NativeScalarStepCharge>,
    need_publications: BTreeMap<crate::task::TaskCorrelation, VecDeque<RuntimeNeedPublication>>,
    latest_need_publications:
        BTreeMap<crate::task::TaskCorrelation, crate::task::RuntimeNeedPublicationRollbackImage>,
    need_publication_frontiers: BTreeMap<crate::task::TaskCorrelation, TaskPublicationCursor>,
    main_started: bool,
    root: Option<RootRuntime>,
    fiber: FlowFiber,
    child_fibers: VecDeque<FlowFiber>,
    next_fiber_id: u64,
    next_scheduled_scope_sequence: u64,
    dialogue_occurrences: BTreeMap<
        (
            RuntimePersistentFiberId,
            crate::runtime_id::RuntimeDialogueContentPlanId,
        ),
        u64,
    >,
    dialogue_activations: dialogue::DialogueActivationStore,
    dialogue_effect_callback_activations:
        BTreeSet<crate::runtime_id::RuntimeDialogueEffectCallbackActivationId>,
    run_child_next: bool,
    pure_i64_batch_inputs: Vec<i64>,
    pure_i64_batch_outputs: Vec<i64>,
    pure_helper_i64_call_shapes: Vec<bool>,
    audio_epoch: u64,
    next_audio_sequence: u64,
    next_host_call_sequence: u64,
}

/// Inert, in-memory rollback of every mutable native Engine owner. Runtime
/// values enter the image through the sealed value snapshot authority; the
/// image may coexist with a live Engine because it carries no live values.
#[derive(Clone, Debug, PartialEq)]
struct NativeEngineRollbackImage {
    evaluation_stats: crate::pure::PureFunctionStats,
    invocation_result: Option<(NativeInvocationRoot, crate::value::AwbcRuntimeValueSnapshot)>,
    plan: Arc<RuntimePlan>,
    format_context: crate::value::RuntimeFormatContext,
    generation: GenerationId,
    need_producers: crate::task::NeedProducerRegistryRollbackImage,
    task_request_quota_remaining: usize,
    scalar_step_charge: Option<NativeScalarStepCharge>,
    need_publications: BTreeMap<
        crate::task::TaskCorrelation,
        VecDeque<crate::task::RuntimeNeedPublicationRollbackImage>,
    >,
    latest_need_publications:
        BTreeMap<crate::task::TaskCorrelation, crate::task::RuntimeNeedPublicationRollbackImage>,
    need_publication_frontiers: BTreeMap<crate::task::TaskCorrelation, TaskPublicationCursor>,
    main_started: bool,
    root: Option<crate::root::RootRuntimeRollbackImage>,
    fiber: FlowFiberRollbackImage,
    child_fibers: VecDeque<FlowFiberRollbackImage>,
    next_fiber_id: u64,
    next_scheduled_scope_sequence: u64,
    dialogue_occurrences: BTreeMap<
        (
            RuntimePersistentFiberId,
            crate::runtime_id::RuntimeDialogueContentPlanId,
        ),
        u64,
    >,
    dialogue_activations: dialogue::DialogueActivationStoreRollbackImage,
    expected_deferred_children: BTreeMap<
        DialogueActivationId,
        (
            crate::runtime_id::RuntimeDeferRegistrationId,
            crate::runtime_id::RuntimeDeferSiteId,
        ),
    >,
    dialogue_effect_callback_activations:
        BTreeSet<crate::runtime_id::RuntimeDialogueEffectCallbackActivationId>,
    run_child_next: bool,
    pure_i64_batch_inputs: Vec<i64>,
    pure_i64_batch_outputs: Vec<i64>,
    pure_helper_i64_call_shapes: Vec<bool>,
    audio_epoch: u64,
    next_audio_sequence: u64,
    next_host_call_sequence: u64,
}

pub(super) struct NativeLineTaskExecutionBatch {
    /// New children staged for commit; already-runnable children stay in Engine.
    child_fibers: VecDeque<FlowFiber>,
    closing_existing: BTreeSet<FlowFiberId>,
    next_fiber_id: u64,
    run_child_next: bool,
    dialogue_effect_callback_activations:
        BTreeSet<crate::runtime_id::RuntimeDialogueEffectCallbackActivationId>,
}

struct NativePreparedLineTaskCommands {
    scheduled: crate::line_task::RuntimeScheduledCompletionStage,
    packets: BTreeMap<
        crate::runtime_id::RuntimeLineHandleToken,
        crate::line_task::RuntimePreparedScheduledPacketTake,
    >,
    next_fiber_id: u64,
}

struct NativePreparedDeferredLineChild {
    type_instantiation: Option<Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    site_id: crate::runtime_id::RuntimeFunctionSiteId,
    capture_tokens: BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
    ordinal: u64,
    allocated: std::num::NonZeroU64,
    next_fiber_id: u64,
}

pub(super) struct NativePreparedDialogueEffectCallback {
    type_instantiation: Option<Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    key: crate::runtime_id::RuntimeDialogueEffectCallbackActivationId,
    ordinal: u64,
    allocated: std::num::NonZeroU64,
    site: crate::runtime_id::RuntimeFunctionSiteId,
    invocation: RuntimeCallableZeroArgInvocationProof,
}

/// Current flow execution cursor.
#[derive(Debug, PartialEq)]
pub struct FlowFiber {
    pub line_cursor: usize,
    pub cursor: Option<FlowCursor>,
    pub pending_ops: VecDeque<FlowOp>,
    pub(crate) control_stack: Vec<FlowControlStackEntry>,
    pub await_observer: Option<Box<AwaitState>>,
    pub root_cleanups: Vec<FlowScopeCleanup>,
    pub env: RuntimeEnv,
    pub observations: RuntimeObservationState,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    /// Terminal cancellation selection awaiting the owning activation transaction.
    pub(crate) selected_dialogue_result: Option<RuntimeValue>,
    pub id: FlowFiberId,
    pub persistent_id: RuntimePersistentFiberId,
    pub execution: crate::runtime_id::ExecutionInstanceId,
    pub(crate) owner: FlowFiberOwner,
    pub status: FlowFiberStatus,
}

#[derive(Clone, Debug, PartialEq)]
struct FlowFiberRollbackImage {
    line_cursor: usize,
    cursor: Option<FlowCursor>,
    pending_ops: VecDeque<FlowOpRollbackImage>,
    control_stack: Vec<FlowControlStackEntryRollbackImage>,
    await_observer: Option<Box<AwaitStateRollbackImage>>,
    root_cleanups: Vec<FlowScopeCleanup>,
    env: crate::value::RuntimeEnvRollbackImage,
    observations: crate::observation::RuntimeObservationSaveSnapshot,
    stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeStateRollbackImage>,
    selected_dialogue_result: Option<crate::value::AwbcRuntimeValueSnapshot>,
    id: FlowFiberId,
    persistent_id: RuntimePersistentFiberId,
    execution: crate::runtime_id::ExecutionInstanceId,
    owner: FlowFiberOwner,
    status: FlowFiberStatusRollbackImage,
}

impl FlowFiber {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<FlowFiberRollbackImage, String> {
        Ok(FlowFiberRollbackImage {
            line_cursor: self.line_cursor,
            cursor: self.cursor,
            pending_ops: self
                .pending_ops
                .iter()
                .map(|op| FlowOpRollbackImage::from_live(op, owner))
                .collect::<Result<_, String>>()?,
            control_stack: self
                .control_stack
                .iter()
                .map(|entry| entry.inert_rollback_image(owner))
                .collect::<Result<_, String>>()?,
            await_observer: self
                .await_observer
                .as_ref()
                .map(|state| state.inert_rollback_image(owner).map(Box::new))
                .transpose()?,
            root_cleanups: self.root_cleanups.clone(),
            env: self.env.inert_rollback_image(owner)?,
            observations:
                crate::observation::RuntimeObservationSaveSnapshot::from_live_for_program(
                    &self.observations,
                    owner,
                )?,
            stream_states: self
                .stream_states
                .iter()
                .map(|(id, state)| Ok((id.clone(), state.inert_rollback_image(owner)?)))
                .collect::<Result<_, String>>()?,
            selected_dialogue_result: self
                .selected_dialogue_result
                .as_ref()
                .map(|value| {
                    crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                        value, owner,
                    )
                    .map_err(|error| error.to_string())
                })
                .transpose()?,
            id: self.id,
            persistent_id: self.persistent_id,
            execution: self.execution,
            owner: self.owner.clone(),
            status: self.status.inert_rollback_image(owner)?,
        })
    }

    fn from_rollback_image(
        image: FlowFiberRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            line_cursor: image.line_cursor,
            cursor: image.cursor,
            pending_ops: image
                .pending_ops
                .into_iter()
                .map(|op| op.into_live(owner))
                .collect::<Result<_, String>>()?,
            control_stack: image
                .control_stack
                .into_iter()
                .map(|entry| FlowControlStackEntry::from_rollback_image(entry, owner))
                .collect::<Result<_, String>>()?,
            await_observer: image
                .await_observer
                .map(|state| AwaitState::from_rollback_image(*state, owner).map(Box::new))
                .transpose()?,
            root_cleanups: image.root_cleanups,
            env: RuntimeEnv::from_rollback_image(image.env, owner)?,
            observations: image.observations.into_live_for_program(owner)?,
            stream_states: image
                .stream_states
                .into_iter()
                .map(|(id, state)| Ok((id, StreamRuntimeState::from_rollback_image(state, owner)?)))
                .collect::<Result<_, String>>()?,
            selected_dialogue_result: image
                .selected_dialogue_result
                .map(|saved| {
                    saved
                        .into_runtime_value_for_program(owner)
                        .map_err(|error| error.to_string())
                })
                .transpose()?,
            id: image.id,
            persistent_id: image.persistent_id,
            execution: image.execution,
            owner: image.owner,
            status: FlowFiberStatus::from_rollback_image(image.status, owner)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
enum FlowOpRollbackImage {
    EnterScheduledScope {
        identity: crate::scope::RuntimeScopeIdentity,
        token: crate::scope::RuntimeScheduledScopeToken,
    },
    ExitScheduledScope {
        token: crate::scope::RuntimeScheduledScopeToken,
    },
    Bind(
        Vec<(
            crate::runtime_id::RuntimeLocalDeclarationId,
            crate::value::AwbcRuntimeValueSnapshot,
        )>,
    ),
    ForNext {
        pattern: RuntimePattern,
        iterator: crate::value::AwbcRuntimeValueSnapshot,
        evidence: crate::plan::RuntimeIteratorEvidence,
        body: Arc<[FlowOp]>,
    },
    CompleteFormatOperand {
        attempt: RuntimeFormatAttemptId,
        parameter: crate::value::RuntimeFmtParameterId,
        ty: RuntimePlanTypeId,
        value: crate::value::AwbcRuntimeValueSnapshot,
    },
    UnevaluatedFormatOperand {
        attempt: RuntimeFormatAttemptId,
        parameter: crate::value::RuntimeFmtParameterId,
        value: RuntimeExpr,
    },
    /// Authored operations are immutable plan rows. Plan admission proves
    /// every embedded literal recursively unrestricted before an operation
    /// can be copied into a native fiber's pending queue.
    Static(FlowOp),
}

impl FlowOpRollbackImage {
    fn from_live(op: &FlowOp, owner: &crate::task::RuntimeProgramOwner) -> Result<Self, String> {
        let image = |value: &RuntimeValue| {
            crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
                .map_err(|error| error.to_string())
        };
        Ok(match op {
            FlowOp::EnterScheduledScope { identity, token } => Self::EnterScheduledScope {
                identity: identity.clone(),
                token: *token,
            },
            FlowOp::ExitScheduledScope { token } => Self::ExitScheduledScope { token: *token },
            FlowOp::Bind(bindings) => Self::Bind(
                bindings
                    .iter()
                    .map(|binding| Ok((binding.local, image(&binding.value)?)))
                    .collect::<Result<_, String>>()?,
            ),
            FlowOp::ForNext {
                pattern,
                iterator,
                evidence,
                body,
            } => Self::ForNext {
                pattern: pattern.clone(),
                iterator:
                    crate::value::AwbcRuntimeValueSnapshot::from_runtime_iterator_for_program(
                        iterator, owner,
                    )
                    .map_err(|error| error.to_string())?,
                evidence: evidence.clone(),
                body: Arc::clone(body),
            },
            FlowOp::CompleteFormatOperand {
                attempt,
                parameter,
                value,
            } => {
                if let crate::value::RuntimeExprKind::Value(value_literal) = value.kind() {
                    Self::CompleteFormatOperand {
                        attempt: *attempt,
                        parameter: *parameter,
                        ty: value.ty(),
                        value: image(value_literal)?,
                    }
                } else if value.literals_permit_copy() {
                    Self::UnevaluatedFormatOperand {
                        attempt: *attempt,
                        parameter: *parameter,
                        value: value.clone(),
                    }
                } else {
                    return Err(
                        "unevaluated formatter completion contains an affine literal".to_owned(),
                    );
                }
            }
            _ if op.literals_permit_copy() => Self::Static(op.clone()),
            _ => return Err("static flow operation contains an affine literal".to_owned()),
        })
    }

    fn into_live(self, owner: &crate::task::RuntimeProgramOwner) -> Result<FlowOp, String> {
        Ok(match self {
            Self::EnterScheduledScope { identity, token } => {
                FlowOp::EnterScheduledScope { identity, token }
            }
            Self::ExitScheduledScope { token } => FlowOp::ExitScheduledScope { token },
            Self::Bind(bindings) => FlowOp::Bind(
                bindings
                    .into_iter()
                    .map(|(local, saved)| {
                        Ok(RuntimeLocalBinding {
                            local,
                            value: saved
                                .into_runtime_value_for_program(owner)
                                .map_err(|error| error.to_string())?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
            ),
            Self::ForNext {
                pattern,
                iterator,
                evidence,
                body,
            } => FlowOp::ForNext {
                pattern,
                iterator: iterator
                    .into_runtime_iterator_for_program(owner)
                    .map_err(|error| error.to_string())?,
                evidence,
                body,
            },
            Self::CompleteFormatOperand {
                attempt,
                parameter,
                ty,
                value,
            } => FlowOp::CompleteFormatOperand {
                attempt,
                parameter,
                value: RuntimeExpr::from_admitted_parts(
                    ty,
                    crate::value::RuntimeExprKind::Value(
                        value
                            .into_runtime_value_for_program(owner)
                            .map_err(|error| error.to_string())?,
                    ),
                ),
            },
            Self::UnevaluatedFormatOperand {
                attempt,
                parameter,
                value,
            } => FlowOp::CompleteFormatOperand {
                attempt,
                parameter,
                value,
            },
            Self::Static(op) => op,
        })
    }
}

/// Stable executor-local identity. It is allocated independently from a
/// plan node so an old child completion cannot be attributed to a later run.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct FlowFiberId(u64);

impl FlowFiberId {
    #[must_use]
    pub(crate) const fn from_executor_ordinal(ordinal: u64) -> Self {
        Self(ordinal)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FlowFiberOwner {
    Executor,
    LineTask(LineTaskFiberOwner),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LineTaskFiberOwner {
    pub(crate) tag: LineTaskWorkTag,
    pub(crate) join_policy: ChildJoinPolicy,
    pub(crate) cancel_policy: ChildCancelPolicy,
    pub(crate) closing: bool,
}

impl FlowFiberOwner {
    fn has_joined_work(&self) -> bool {
        !matches!(
            self,
            Self::LineTask(LineTaskFiberOwner {
                join_policy: ChildJoinPolicy::Detached,
                ..
            })
        )
    }

    fn requests_line_task_close(&self) -> bool {
        matches!(
            self,
            Self::LineTask(LineTaskFiberOwner { closing: true, .. })
        )
    }
}

fn flow_fiber_line_handle_tokens(
    fiber: &FlowFiber,
) -> Result<BTreeSet<crate::runtime_id::RuntimeLineHandleToken>, crate::line_task::LineRuntimeError>
{
    let mut tokens = flow_fiber_line_handle_owners(fiber)?
        .into_keys()
        .collect::<BTreeSet<_>>();
    if let Some(selected) = &fiber.selected_dialogue_result {
        for handle in selected
            .affine_line_handles()
            .map_err(|_| crate::line_task::LineRuntimeError::InvalidHandlePayload)?
        {
            tokens.insert(handle.token().clone());
        }
    }
    Ok(tokens)
}

fn flow_fiber_line_handle_owners(
    fiber: &FlowFiber,
) -> Result<
    BTreeMap<
        crate::runtime_id::RuntimeLineHandleToken,
        crate::value::ownership::RuntimeOwnedSlotId,
    >,
    crate::line_task::LineRuntimeError,
> {
    let mut owners = BTreeMap::new();
    for (local, value) in fiber.env.bindings() {
        let owner =
            crate::value::ownership::RuntimeOwnedSlotId::environment_local(fiber.execution, local);
        for handle in value
            .affine_line_handles()
            .map_err(|_| crate::line_task::LineRuntimeError::InvalidHandlePayload)?
        {
            if owners.insert(handle.token().clone(), owner).is_some() {
                return Err(crate::line_task::LineRuntimeError::DuplicateHandleOccurrence);
            }
        }
    }
    for (frame_index, entry) in fiber.control_stack.iter().enumerate() {
        let FlowControlStackEntryKind::FormatAttempt(frame) = &entry.kind else {
            continue;
        };
        let frame_index = u32::try_from(frame_index)
            .map_err(|_| crate::line_task::LineRuntimeError::OwnedSlotOverflow)?;
        for (ordinal, value) in frame.values.iter().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| crate::line_task::LineRuntimeError::OwnedSlotOverflow)?;
            let owner = crate::value::ownership::RuntimeOwnedSlotId::NativeFormatOperand {
                execution: fiber.execution,
                fiber: fiber.persistent_id,
                frame: frame_index,
                attempt: frame.attempt,
                ordinal,
            };
            for handle in value
                .affine_line_handles()
                .map_err(|_| crate::line_task::LineRuntimeError::InvalidHandlePayload)?
            {
                if owners.insert(handle.token().clone(), owner).is_some() {
                    return Err(crate::line_task::LineRuntimeError::DuplicateHandleOccurrence);
                }
            }
        }
    }
    Ok(owners)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowControlStackEntry {
    pub(crate) kind: FlowControlStackEntryKind,
}

/// One deterministic cleanup effect registered against a lexical flow scope.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowScopeCleanup {
    pub key: String,
    pub effect: LineEffectRequest,
}

impl FlowScopeCleanup {
    pub fn new(key: impl Into<String>, effect: LineEffectRequest) -> Self {
        Self {
            key: key.into(),
            effect,
        }
    }
}

/// Structured frame kind for the minimal flow executor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FlowControlStackEntryKind {
    Scope {
        origin: crate::scope::RuntimeScopeFrameOrigin,
        cleanups: Vec<FlowScopeCleanup>,
        match_guard: Option<flow::match_guard::NativeMatchGuardContinuation>,
    },
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
        guard: Option<Box<RuntimeExpr>>,
        body: std::sync::Arc<[FlowOp]>,
    },
    /// One source-ordered formatter occurrence. The frame remains the sole
    /// outcome authority until its matching FormatContent expression consumes it.
    FormatAttempt(NativeFormatAttemptFrame),
    /// Typed function-call return boundary. The callee's scope and any nested
    /// loop/scope frames are unwound before the returned value is admitted to
    /// the caller's result pattern.
    FunctionCall(FunctionCallFrame),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeFormatAttemptFrame {
    pub(crate) attempt: RuntimeFormatAttemptId,
    pub(crate) context: crate::value::RuntimeFormatContext,
    pub(crate) values: Vec<Option<RuntimeValue>>,
    pub(crate) first_recoverable: Option<String>,
    pub(crate) next_operand: usize,
    pub(crate) active: Option<NativeFormatOperandFrame>,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeFormatAttemptRollbackImage {
    attempt: RuntimeFormatAttemptId,
    context: crate::value::RuntimeFormatContext,
    values: Vec<Option<crate::value::AwbcRuntimeValueSnapshot>>,
    first_recoverable: Option<String>,
    next_operand: usize,
    active: Option<NativeFormatOperandRollbackImage>,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeFormatOperandRollbackImage {
    ordinal: usize,
    resume: Option<FlowCursor>,
    caller_pending_ops: VecDeque<FlowOpRollbackImage>,
}

impl NativeFormatAttemptFrame {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<NativeFormatAttemptRollbackImage, String> {
        Ok(NativeFormatAttemptRollbackImage {
            attempt: self.attempt,
            context: self.context.clone(),
            values: self
                .values
                .iter()
                .map(|value| {
                    value
                        .as_ref()
                        .map(|value| {
                            crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                                value, owner,
                            )
                            .map_err(|error| error.to_string())
                        })
                        .transpose()
                })
                .collect::<Result<_, String>>()?,
            first_recoverable: self.first_recoverable.clone(),
            next_operand: self.next_operand,
            active: self
                .active
                .as_ref()
                .map(
                    |active| -> Result<NativeFormatOperandRollbackImage, String> {
                        Ok(NativeFormatOperandRollbackImage {
                            ordinal: active.ordinal,
                            resume: active.resume,
                            caller_pending_ops: active
                                .caller_pending_ops
                                .iter()
                                .map(|op| FlowOpRollbackImage::from_live(op, owner))
                                .collect::<Result<_, String>>()?,
                        })
                    },
                )
                .transpose()?,
        })
    }

    fn from_rollback_image(
        image: NativeFormatAttemptRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            attempt: image.attempt,
            context: image.context,
            values: image
                .values
                .into_iter()
                .map(|value| {
                    value
                        .map(|saved| {
                            saved
                                .into_runtime_value_for_program(owner)
                                .map_err(|error| error.to_string())
                        })
                        .transpose()
                })
                .collect::<Result<_, String>>()?,
            first_recoverable: image.first_recoverable,
            next_operand: image.next_operand,
            active: image
                .active
                .map(|active| -> Result<NativeFormatOperandFrame, String> {
                    Ok(NativeFormatOperandFrame {
                        ordinal: active.ordinal,
                        resume: active.resume,
                        caller_pending_ops: active
                            .caller_pending_ops
                            .into_iter()
                            .map(|op| op.into_live(owner))
                            .collect::<Result<_, String>>()?,
                    })
                })
                .transpose()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeFormatOperandFrame {
    pub(crate) ordinal: usize,
    pub(crate) resume: Option<FlowCursor>,
    pub(crate) caller_pending_ops: VecDeque<FlowOp>,
}

/// One invocation frame shared by direct and value-based function calls.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FunctionCallFrame {
    site: crate::runtime_id::RuntimeFunctionSiteId,
    type_instantiation: Option<Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    function_scope: bool,
    resume: Option<FlowCursor>,
    caller_pending_ops: VecDeque<FlowOp>,
    continuation: FunctionReturnContinuation,
}

#[derive(Clone, Debug, PartialEq)]
enum FlowControlStackEntryRollbackImage {
    Static(FlowControlStackEntryKind),
    FormatAttempt(NativeFormatAttemptRollbackImage),
    FunctionCall {
        site: crate::runtime_id::RuntimeFunctionSiteId,
        type_instantiation: Option<Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
        function_scope: bool,
        resume: Option<FlowCursor>,
        caller_pending_ops: VecDeque<FlowOpRollbackImage>,
        continuation: FunctionReturnContinuationRollbackImage,
    },
}

impl FlowControlStackEntry {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<FlowControlStackEntryRollbackImage, String> {
        Ok(match &self.kind {
            FlowControlStackEntryKind::FormatAttempt(frame) => {
                FlowControlStackEntryRollbackImage::FormatAttempt(
                    frame.inert_rollback_image(owner)?,
                )
            }
            FlowControlStackEntryKind::FunctionCall(frame) => {
                FlowControlStackEntryRollbackImage::FunctionCall {
                    site: frame.site,
                    type_instantiation: frame.type_instantiation.clone(),
                    function_scope: frame.function_scope,
                    resume: frame.resume,
                    caller_pending_ops: frame
                        .caller_pending_ops
                        .iter()
                        .map(|op| FlowOpRollbackImage::from_live(op, owner))
                        .collect::<Result<_, String>>()?,
                    continuation: frame.continuation.inert_rollback_image(owner)?,
                }
            }
            _ => FlowControlStackEntryRollbackImage::Static(self.kind.clone()),
        })
    }

    fn from_rollback_image(
        image: FlowControlStackEntryRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let kind = match image {
            FlowControlStackEntryRollbackImage::Static(kind) => kind,
            FlowControlStackEntryRollbackImage::FormatAttempt(frame) => {
                FlowControlStackEntryKind::FormatAttempt(
                    NativeFormatAttemptFrame::from_rollback_image(frame, owner)?,
                )
            }
            FlowControlStackEntryRollbackImage::FunctionCall {
                site,
                type_instantiation,
                function_scope,
                resume,
                caller_pending_ops,
                continuation,
            } => {
                let crate::task::RuntimeProgramOwner::Plan(plan) = owner else {
                    return Err("native function frame has a foreign program kind".to_owned());
                };
                plan.validate_function_instantiation(site, type_instantiation.as_deref())
                    .map_err(|error| error.to_string())?;
                FlowControlStackEntryKind::FunctionCall(FunctionCallFrame {
                    site,
                    type_instantiation,
                    function_scope,
                    resume,
                    caller_pending_ops: caller_pending_ops
                        .into_iter()
                        .map(|op| op.into_live(owner))
                        .collect::<Result<_, String>>()?,
                    continuation: FunctionReturnContinuation::from_rollback_image(
                        continuation,
                        site,
                        owner,
                    )?,
                })
            }
        };
        Ok(Self { kind })
    }
}

impl FunctionCallFrame {
    fn new(
        site: crate::runtime_id::RuntimeFunctionSiteId,
        resume: Option<FlowCursor>,
        continuation: FunctionReturnContinuation,
    ) -> Self {
        Self {
            site,
            type_instantiation: None,
            function_scope: false,
            resume,
            caller_pending_ops: VecDeque::new(),
            continuation,
        }
    }
}

/// The caller operation resumed after an invocation returns. A default
/// resumes its materialization plan without reevaluating source operands.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FunctionReturnContinuation {
    Program {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    },
    Function {
        site: crate::runtime_id::RuntimeFunctionSiteId,
    },
    CallableDefault {
        pending: crate::value::RuntimeCallablePendingGroup,
        result: RuntimePattern,
    },
    Bind {
        result: RuntimePattern,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum FunctionReturnContinuationRollbackImage {
    Program {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    },
    Function {
        site: crate::runtime_id::RuntimeFunctionSiteId,
    },
    CallableDefault {
        pending: crate::value::RuntimeCallablePendingRollbackImage,
        result: RuntimePattern,
    },
    Bind {
        result: RuntimePattern,
    },
}

impl FunctionReturnContinuation {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<FunctionReturnContinuationRollbackImage, String> {
        Ok(match self {
            Self::Program { program } => {
                FunctionReturnContinuationRollbackImage::Program { program: *program }
            }
            Self::Function { site } => {
                FunctionReturnContinuationRollbackImage::Function { site: *site }
            }
            Self::CallableDefault { pending, result } => {
                FunctionReturnContinuationRollbackImage::CallableDefault {
                    pending: pending.inert_rollback_image(owner)?,
                    result: result.clone(),
                }
            }
            Self::Bind { result } => FunctionReturnContinuationRollbackImage::Bind {
                result: result.clone(),
            },
        })
    }

    fn from_rollback_image(
        image: FunctionReturnContinuationRollbackImage,
        site: crate::runtime_id::RuntimeFunctionSiteId,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(match image {
            FunctionReturnContinuationRollbackImage::Program { program } => {
                let crate::task::RuntimeProgramOwner::Plan(plan) = owner else {
                    return Err("native program continuation requires its plan owner".to_owned());
                };
                if !plan
                    .resolve_pure_program(program)
                    .is_ok_and(|binding| binding.site() == site)
                {
                    return Err("native program continuation is absent from its plan".to_owned());
                }
                Self::Program { program }
            }
            FunctionReturnContinuationRollbackImage::Function { site: target } => {
                let crate::task::RuntimeProgramOwner::Plan(plan) = owner else {
                    return Err("native function continuation requires its plan owner".to_owned());
                };
                if target != site
                    || crate::pure::RuntimePureFunctionRef::resolve(plan, target).is_err()
                {
                    return Err("native function continuation is absent from its plan".to_owned());
                }
                Self::Function { site: target }
            }
            FunctionReturnContinuationRollbackImage::CallableDefault { pending, result } => {
                Self::CallableDefault {
                    pending: crate::value::RuntimeCallablePendingGroup::from_rollback_image(
                        pending, owner,
                    )?,
                    result,
                }
            }
            FunctionReturnContinuationRollbackImage::Bind { result } => Self::Bind { result },
        })
    }
}

/// Position in a lowered flow program.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlowCursor {
    pub flow_index: usize,
    pub op_index: usize,
}

/// Failure to select an explicit flow program before execution.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EngineStartError {
    #[error("runtime flow `{flow}` does not exist")]
    MissingFlow { flow: String },
    #[error("runtime entry `{entry}` does not exist")]
    MissingEntry { entry: String },
    #[error("runtime entry `{entry}` does not select one flow")]
    EntryDoesNotSelectFlow { entry: String },
    #[error("runtime engine already has a selected flow")]
    AlreadyStarted,
    #[error("runtime Flow invocation is invalid: {message}")]
    InvalidFlowInvocation { message: String },
    #[error("runtime entry `{entry}` failed root startup validation: {message}")]
    InvalidRootStartup { entry: String, message: String },
}

struct StructuredRootEvaluator<'a> {
    plan: &'a Arc<RuntimePlan>,
    scratch: VmPureFunctionScratch,
}

impl<'a> StructuredRootEvaluator<'a> {
    fn new(plan: &'a Arc<RuntimePlan>) -> Self {
        Self {
            plan,
            scratch: VmPureFunctionScratch::default(),
        }
    }
}

impl RootCallableEvaluator for StructuredRootEvaluator<'_> {
    fn evaluate_root_callable(
        &mut self,
        callable: &RuntimeCallableRole,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RootCallableEvaluationError> {
        let executable = self
            .plan
            .callable_executables
            .iter()
            .find(|executable| {
                executable.callable == callable.callable && executable.contract == callable.contract
            })
            .ok_or_else(|| {
                RootCallableEvaluationError::new(format!(
                    "missing executable callable `{}`",
                    callable.callable.as_str()
                ))
            })?;
        match executable.code {
            RuntimeCallableExecutableCode::PureHelper(helper) => self
                .scratch
                .evaluate_values(self.plan, helper, args)
                .map_err(|error| RootCallableEvaluationError::new(error.to_string())),
            RuntimeCallableExecutableCode::FunctionSite(site) => self
                .scratch
                .evaluate_function_site(self.plan, site, args)
                .map_err(|error| RootCallableEvaluationError::new(error.to_string())),
            RuntimeCallableExecutableCode::ControllerFlow(_) => {
                Err(RootCallableEvaluationError::new(format!(
                    "callable `{}` is not a pure root callable",
                    callable.callable.as_str()
                )))
            }
        }
    }
}

/// Execution phase of the sole dialogue activation owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueRuntimePhase {
    Activating,
    Ready,
    Closing,
    Publishing,
}

/// Published projection of a suspended dialogue activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueExecutionStatus {
    activation: DialogueActivationId,
    phase: DialogueRuntimePhase,
}

impl DialogueExecutionStatus {
    pub(crate) const fn new(activation: DialogueActivationId, phase: DialogueRuntimePhase) -> Self {
        Self { activation, phase }
    }

    #[must_use]
    pub const fn activation(&self) -> &DialogueActivationId {
        &self.activation
    }

    #[must_use]
    pub const fn phase(&self) -> DialogueRuntimePhase {
        self.phase
    }

    /// Only the reveal phase accepts presentation progression.
    #[must_use]
    pub const fn waiting_presentation_activation(&self) -> Option<&DialogueActivationId> {
        match self.phase {
            DialogueRuntimePhase::Ready => Some(&self.activation),
            DialogueRuntimePhase::Activating
            | DialogueRuntimePhase::Closing
            | DialogueRuntimePhase::Publishing => None,
        }
    }
}

/// High-level flow status for the minimal runtime spine.
#[derive(Debug, PartialEq)]
pub enum FlowFiberStatus {
    Running,
    Dialogue(DialogueExecutionStatus),
    NeedWaiting(Box<AwaitState>),
    WaitingMany(WaitingManyStatus),
    HostCall(HostCallState),
    Choice(ChoiceState),
    Done(FlowExit),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
enum FlowFiberStatusRollbackImage {
    Running,
    Dialogue(DialogueExecutionStatus),
    NeedWaiting(Box<AwaitStateRollbackImage>),
    WaitingManyNative(Box<AwaitManyStateRollbackImage>),
    WaitingManyObserved(AwaitManyProgress),
    HostCall(HostCallState),
    Choice(ChoiceState),
    Done(FlowExit),
    Failed(String),
}

impl FlowFiberStatus {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<FlowFiberStatusRollbackImage, String> {
        Ok(match self {
            Self::Running => FlowFiberStatusRollbackImage::Running,
            Self::Dialogue(activation) => {
                FlowFiberStatusRollbackImage::Dialogue(activation.clone())
            }
            Self::NeedWaiting(state) => FlowFiberStatusRollbackImage::NeedWaiting(Box::new(
                state.inert_rollback_image(owner)?,
            )),
            Self::WaitingMany(WaitingManyStatus::Native(state)) => {
                FlowFiberStatusRollbackImage::WaitingManyNative(Box::new(
                    state.inert_rollback_image(owner)?,
                ))
            }
            Self::WaitingMany(WaitingManyStatus::Observed(progress)) => {
                FlowFiberStatusRollbackImage::WaitingManyObserved(progress.clone())
            }
            Self::HostCall(state) => FlowFiberStatusRollbackImage::HostCall(state.clone()),
            Self::Choice(state) => FlowFiberStatusRollbackImage::Choice(state.clone()),
            Self::Done(exit) => FlowFiberStatusRollbackImage::Done(exit.clone()),
            Self::Failed(message) => FlowFiberStatusRollbackImage::Failed(message.clone()),
        })
    }

    fn from_rollback_image(
        image: FlowFiberStatusRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(match image {
            FlowFiberStatusRollbackImage::Running => Self::Running,
            FlowFiberStatusRollbackImage::Dialogue(activation) => Self::Dialogue(activation),
            FlowFiberStatusRollbackImage::NeedWaiting(state) => {
                Self::NeedWaiting(Box::new(AwaitState::from_rollback_image(*state, owner)?))
            }
            FlowFiberStatusRollbackImage::WaitingManyNative(state) => {
                Self::WaitingMany(WaitingManyStatus::Native(Box::new(
                    AwaitManyState::from_rollback_image(*state, owner)?,
                )))
            }
            FlowFiberStatusRollbackImage::WaitingManyObserved(progress) => {
                Self::WaitingMany(WaitingManyStatus::Observed(progress))
            }
            FlowFiberStatusRollbackImage::HostCall(state) => Self::HostCall(state),
            FlowFiberStatusRollbackImage::Choice(state) => Self::Choice(state),
            FlowFiberStatusRollbackImage::Done(exit) => Self::Done(exit),
            FlowFiberStatusRollbackImage::Failed(message) => Self::Failed(message),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct StreamRuntimeStateRollbackImage {
    id: StreamRuntimeId,
    queue: VecDeque<crate::value::AwbcRuntimeValueSnapshot>,
    closed: bool,
    emitted_count: u64,
}

impl StreamRuntimeState {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<StreamRuntimeStateRollbackImage, String> {
        Ok(StreamRuntimeStateRollbackImage {
            id: self.id.clone(),
            queue: self
                .queue
                .iter()
                .map(|value| {
                    crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                        value.value(),
                        owner,
                    )
                    .map_err(|error| error.to_string())
                })
                .collect::<Result<_, String>>()?,
            closed: self.closed,
            emitted_count: self.emitted_count,
        })
    }

    fn from_rollback_image(
        image: StreamRuntimeStateRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            id: image.id,
            queue: image
                .queue
                .into_iter()
                .map(|saved| {
                    saved
                        .into_runtime_value_for_program(owner)
                        .map(RuntimePayload)
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<_, String>>()?,
            closed: image.closed,
            emitted_count: image.emitted_count,
        })
    }
}

/// String presentation style for high-level runtime flow status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowStatusLabelStyle {
    /// Stable runtime-facing status used by host/bundle/player observations.
    Runtime,
    /// Existing CLI/debug spelling that preserves `FlowExit`'s `Debug` form.
    Debug,
    /// Coarse status used when only the status kind should be exposed.
    Compact,
}

/// Suspended `await ... with` state.
#[derive(Debug, PartialEq)]
pub struct AwaitState {
    pub binding: Option<RuntimePattern>,
    pub handle: crate::task::RuntimeNeedHandle,
    pub item_type: AwaitItemType,
    pub observers: Vec<crate::plan::RuntimeAwaitPendingObserver>,
    pub resume: Option<FlowCursor>,
    pub observed_through: Option<TaskPublicationCursor>,
    pub queued: VecDeque<RuntimeNeedPublication>,
}

#[derive(Clone, Debug, PartialEq)]
struct AwaitStateRollbackImage {
    binding: Option<RuntimePattern>,
    handle: crate::task::RuntimeNeedHandleSaveSnapshot,
    item_type: AwaitItemType,
    observers: Vec<crate::plan::RuntimeAwaitPendingObserver>,
    resume: Option<FlowCursor>,
    observed_through: Option<TaskPublicationCursor>,
    queued: VecDeque<crate::task::RuntimeNeedPublicationRollbackImage>,
}

impl AwaitState {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<AwaitStateRollbackImage, String> {
        Ok(AwaitStateRollbackImage {
            binding: self.binding.clone(),
            handle: crate::task::RuntimeNeedHandleSaveSnapshot::from_live(
                &self.handle,
                Some(owner),
            )
            .map_err(|error| error.to_string())?,
            item_type: self.item_type,
            observers: self.observers.clone(),
            resume: self.resume,
            observed_through: self.observed_through,
            queued: self
                .queued
                .iter()
                .map(|publication| publication.inert_rollback_image(owner))
                .collect::<Result<_, String>>()?,
        })
    }

    fn from_rollback_image(
        image: AwaitStateRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            binding: image.binding,
            handle: image
                .handle
                .into_live(owner)
                .map_err(|error| error.to_string())?,
            item_type: image.item_type,
            observers: image.observers,
            resume: image.resume,
            observed_through: image.observed_through,
            queued: image
                .queued
                .into_iter()
                .map(|publication| RuntimeNeedPublication::from_rollback_image(publication, owner))
                .collect::<Result<_, String>>()?,
        })
    }
}

/// Typed Ready payload identity carried by a suspended Await across execution
/// tiers. Plan and AWBC type IDs are kept in their owning domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwaitItemType {
    Plan(RuntimePlanTypeId),
    Awbc(AwbcTypeId),
}

/// Suspended bounded fanout await state.
#[derive(Debug, PartialEq)]
pub struct AwaitManyState {
    pub binding: Option<RuntimePattern>,
    pub target: AwaitManyTarget,
    pub base: crate::task::RuntimeNeedHandle,
    pub captured: Vec<RuntimeValue>,
    pub resume: Option<FlowCursor>,
    pub items: Vec<RuntimeValue>,
    pub next_index: usize,
    pub in_flight: Vec<AwaitManyInFlight>,
    pub results: Vec<Option<RuntimePayload>>,
}

#[derive(Clone, Debug, PartialEq)]
struct AwaitManyStateRollbackImage {
    binding: Option<RuntimePattern>,
    target: AwaitManyTarget,
    base: crate::task::RuntimeNeedHandleSaveSnapshot,
    captured: Vec<crate::value::AwbcRuntimeValueSnapshot>,
    resume: Option<FlowCursor>,
    items: Vec<crate::value::AwbcRuntimeValueSnapshot>,
    next_index: usize,
    in_flight: Vec<AwaitManyInFlight>,
    results: Vec<Option<crate::value::AwbcRuntimeValueSnapshot>>,
}

impl AwaitManyState {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<AwaitManyStateRollbackImage, String> {
        let image = |value: &RuntimeValue| {
            crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
                .map_err(|error| error.to_string())
        };
        Ok(AwaitManyStateRollbackImage {
            binding: self.binding.clone(),
            target: self.target.clone(),
            base: crate::task::RuntimeNeedHandleSaveSnapshot::from_live(&self.base, Some(owner))
                .map_err(|error| error.to_string())?,
            captured: self.captured.iter().map(image).collect::<Result<_, _>>()?,
            resume: self.resume,
            items: self
                .items
                .iter()
                .map(image)
                .collect::<Result<_, String>>()?,
            next_index: self.next_index,
            in_flight: self.in_flight.clone(),
            results: self
                .results
                .iter()
                .map(|result| {
                    result
                        .as_ref()
                        .map(|value| image(value.value()))
                        .transpose()
                })
                .collect::<Result<_, String>>()?,
        })
    }

    fn from_rollback_image(
        image: AwaitManyStateRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let value = |saved: crate::value::AwbcRuntimeValueSnapshot| {
            saved
                .into_runtime_value_for_program(owner)
                .map_err(|error| error.to_string())
        };
        Ok(Self {
            binding: image.binding,
            target: image.target,
            base: image
                .base
                .into_live(owner)
                .map_err(|error| error.to_string())?,
            captured: image
                .captured
                .into_iter()
                .map(value)
                .collect::<Result<_, _>>()?,
            resume: image.resume,
            items: image
                .items
                .into_iter()
                .map(value)
                .collect::<Result<_, String>>()?,
            next_index: image.next_index,
            in_flight: image.in_flight,
            results: image
                .results
                .into_iter()
                .map(|result| {
                    result
                        .map(|saved| value(saved).map(RuntimePayload))
                        .transpose()
                })
                .collect::<Result<_, String>>()?,
        })
    }
}

/// One in-flight child task inside a bounded fanout await.
#[derive(Clone, Debug, PartialEq)]
pub struct AwaitManyInFlight {
    pub index: usize,
    pub handle: crate::task::RuntimeNeedHandle,
}

/// Shared status projection for both native and Product AwaitMany execution.
/// Native retains its complete continuation; Product reports a display-only
/// summary while its verified AWBC fiber remains the continuation authority.
#[derive(Debug, PartialEq)]
pub enum WaitingManyStatus {
    Native(Box<AwaitManyState>),
    Observed(AwaitManyProgress),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitManyProgress {
    pub task: TaskId,
    pub completed: usize,
    pub total: usize,
}

impl WaitingManyStatus {
    fn runtime_status_label(&self) -> String {
        match self {
            Self::Native(state) => format!(
                "waiting_many {} {}/{}",
                state.base.correlation().task_id,
                state.results.iter().filter(|value| value.is_some()).count(),
                state.results.len()
            ),
            Self::Observed(progress) => format!(
                "waiting_many {} {}/{}",
                progress.task, progress.completed, progress.total
            ),
        }
    }

    fn debug_status_label(&self) -> String {
        self.runtime_status_label()
    }
}

/// Suspended direct host call awaiting a typed host result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostCallResultType {
    Plan(RuntimePlanTypeId),
    Awbc(AwbcTypeId),
    Unit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostCallState {
    pub binding: Option<RuntimePattern>,
    pub id: RuntimeHostCallId,
    pub result_type: HostCallResultType,
    pub resume: Option<FlowCursor>,
}

/// Suspended choice state.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceState {
    pub id: Option<String>,
    pub options: Vec<ChoiceRuntimeOption>,
    pub resume: Option<FlowCursor>,
}

/// Terminal flow result observed by the minimal runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowExit {
    Done,
    Return(String),
}

impl FlowFiberStatus {
    pub fn status_label(&self, style: FlowStatusLabelStyle) -> String {
        match style {
            FlowStatusLabelStyle::Runtime => self.runtime_status_label(),
            FlowStatusLabelStyle::Debug => self.debug_status_label(),
            FlowStatusLabelStyle::Compact => self.compact_status_label(),
        }
    }

    fn runtime_status_label(&self) -> String {
        match self {
            Self::Running => "running".to_owned(),
            Self::Dialogue(_) => "dialogue".to_owned(),
            Self::NeedWaiting(state) => format!("need_waiting {}", state.handle.need_id()),
            Self::WaitingMany(state) => state.runtime_status_label(),
            Self::HostCall(state) => format!("host_call {}", state.id.0),
            Self::Choice(state) => {
                format!("choice {}", state.id.as_deref().unwrap_or("-"))
            }
            Self::Done(exit) => exit.runtime_status_label(),
            Self::Failed(message) => format!("failed {message}"),
        }
    }

    fn debug_status_label(&self) -> String {
        match self {
            Self::Running => "running".to_owned(),
            Self::Dialogue(_) => "dialogue".to_owned(),
            Self::NeedWaiting(state) => format!("need_waiting {}", state.handle.need_id()),
            Self::WaitingMany(state) => state.debug_status_label(),
            Self::HostCall(state) => format!("host_call {}", state.id.0),
            Self::Choice(state) => {
                format!("choice {}", state.id.as_deref().unwrap_or("-"))
            }
            Self::Done(exit) => format!("done {exit:?}"),
            Self::Failed(message) => format!("failed {message}"),
        }
    }

    fn compact_status_label(&self) -> String {
        match self {
            Self::Running => "running".to_owned(),
            Self::Dialogue(_) => "dialogue".to_owned(),
            Self::NeedWaiting(_) => "need_waiting".to_owned(),
            Self::WaitingMany(_) => "waiting_many".to_owned(),
            Self::HostCall(_) => "host_call".to_owned(),
            Self::Choice(_) => "choice".to_owned(),
            Self::Done(exit) => exit.compact_status_label(),
            Self::Failed(message) => format!("failed:{message}"),
        }
    }
}

impl FlowExit {
    fn runtime_status_label(&self) -> String {
        match self {
            Self::Done => "done".to_owned(),
            Self::Return(value) => format!("done return {value}"),
        }
    }

    fn compact_status_label(&self) -> String {
        match self {
            Self::Done => "done".to_owned(),
            Self::Return(value) => format!("done return {value}"),
        }
    }
}

impl Default for FlowFiber {
    fn default() -> Self {
        Self {
            line_cursor: 0,
            cursor: None,
            pending_ops: VecDeque::new(),
            control_stack: Vec::new(),
            await_observer: None,
            root_cleanups: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            stream_states: BTreeMap::new(),
            selected_dialogue_result: None,
            id: FlowFiberId::default(),
            persistent_id: RuntimePersistentFiberId::default(),
            execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                std::num::NonZeroU64::MIN,
            ),
            owner: FlowFiberOwner::Executor,
            status: FlowFiberStatus::Done(FlowExit::Done),
        }
    }
}

fn pure_helper_i64_call_shapes(plan: &RuntimePlan) -> Vec<bool> {
    plan.pure_helpers
        .iter()
        .map(eval::pure_helper_has_i64_call_shape)
        .collect()
}

impl Engine {
    fn inert_rollback_image(&self) -> Result<NativeEngineRollbackImage, String> {
        let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan));
        let mut expected_deferred_children = BTreeMap::new();
        for child in &self.child_fibers {
            let FlowFiberOwner::LineTask(child_owner) = &child.owner else {
                continue;
            };
            let crate::line_task::LineTaskWork::Defer(id) = child_owner.tag.work() else {
                continue;
            };
            let activation = child_owner.tag.activation_id();
            let Some(line) = self.dialogue_activations.active_line(activation) else {
                continue;
            };
            let Some((inflight_id, site)) = line.deferred_inflight() else {
                // Scoped defers have a frame-owned in-flight marker instead.
                continue;
            };
            if inflight_id != id
                || expected_deferred_children
                    .insert(activation.clone(), (id, site))
                    .is_some()
            {
                return Err("native deferred child custody is inconsistent".to_owned());
            }
        }
        let publication_image =
            |publication: &RuntimeNeedPublication| publication.inert_rollback_image(&owner);
        Ok(NativeEngineRollbackImage {
            evaluation_stats: self.evaluation_stats.clone(),
            invocation_result: self
                .invocation_result
                .as_ref()
                .map(|(program, value)| {
                    crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                        value, &owner,
                    )
                    .map(|snapshot| (*program, snapshot))
                    .map_err(|error| error.to_string())
                })
                .transpose()?,
            plan: Arc::clone(&self.plan),
            format_context: self.format_context.clone(),
            generation: self.generation,
            need_producers: self.need_producers.inert_rollback_image(&owner)?,
            task_request_quota_remaining: self.task_request_quota_remaining,
            scalar_step_charge: self.scalar_step_charge,
            need_publications: self
                .need_publications
                .iter()
                .map(|(need, queue)| {
                    Ok((
                        need.clone(),
                        queue
                            .iter()
                            .map(publication_image)
                            .collect::<Result<_, String>>()?,
                    ))
                })
                .collect::<Result<_, String>>()?,
            latest_need_publications: self.latest_need_publications.clone(),
            need_publication_frontiers: self.need_publication_frontiers.clone(),
            main_started: self.main_started,
            root: self
                .root
                .as_ref()
                .map(|root| root.inert_rollback_image(&owner))
                .transpose()?,
            fiber: self.fiber.inert_rollback_image(&owner)?,
            child_fibers: self
                .child_fibers
                .iter()
                .map(|child| child.inert_rollback_image(&owner))
                .collect::<Result<_, String>>()?,
            next_fiber_id: self.next_fiber_id,
            next_scheduled_scope_sequence: self.next_scheduled_scope_sequence,
            dialogue_occurrences: self.dialogue_occurrences.clone(),
            dialogue_activations: self.dialogue_activations.inert_rollback_image(&owner)?,
            expected_deferred_children,
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
            run_child_next: self.run_child_next,
            pure_i64_batch_inputs: self.pure_i64_batch_inputs.clone(),
            pure_i64_batch_outputs: self.pure_i64_batch_outputs.clone(),
            pure_helper_i64_call_shapes: self.pure_helper_i64_call_shapes.clone(),
            audio_epoch: self.audio_epoch,
            next_audio_sequence: self.next_audio_sequence,
            next_host_call_sequence: self.next_host_call_sequence,
        })
    }

    fn from_rollback_image(image: NativeEngineRollbackImage) -> Result<Self, String> {
        let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&image.plan));
        let restored = Self {
            evaluation_stats: image.evaluation_stats,
            invocation_result: image
                .invocation_result
                .map(|(root, value)| {
                    let value = value
                        .into_runtime_value_for_program(&owner)
                        .map_err(|error| error.to_string())?;
                    if let NativeInvocationRoot::Function(site) = root {
                        let function =
                            crate::pure::RuntimePureFunctionRef::resolve(&image.plan, site)
                                .map_err(|error| error.to_string())?;
                        if !image
                            .plan
                            .value_matches_type(function.result_type(), &value)
                            .map_err(|error| error.to_string())?
                        {
                            return Err(
                                "native function result disagrees with its admitted return type"
                                    .to_owned(),
                            );
                        }
                    }
                    Ok((root, value))
                })
                .transpose()?,
            plan: image.plan,
            format_context: image.format_context,
            generation: image.generation,
            need_producers: NeedProducerRegistry::from_rollback_image(
                image.need_producers,
                &owner,
            )?,
            task_request_quota_remaining: image.task_request_quota_remaining,
            scalar_step_charge: image.scalar_step_charge,
            need_publications: image
                .need_publications
                .into_iter()
                .map(|(need, queue)| {
                    Ok((
                        need,
                        queue
                            .into_iter()
                            .map(|publication| {
                                RuntimeNeedPublication::from_rollback_image(publication, &owner)
                            })
                            .collect::<Result<_, String>>()?,
                    ))
                })
                .collect::<Result<_, String>>()?,
            latest_need_publications: image.latest_need_publications,
            need_publication_frontiers: image.need_publication_frontiers,
            main_started: image.main_started,
            root: image
                .root
                .map(|root| RootRuntime::from_rollback_image(root, &owner))
                .transpose()?,
            fiber: FlowFiber::from_rollback_image(image.fiber, &owner)?,
            child_fibers: image
                .child_fibers
                .into_iter()
                .map(|child| FlowFiber::from_rollback_image(child, &owner))
                .collect::<Result<_, String>>()?,
            next_fiber_id: image.next_fiber_id,
            next_scheduled_scope_sequence: image.next_scheduled_scope_sequence,
            dialogue_occurrences: image.dialogue_occurrences,
            dialogue_activations: dialogue::DialogueActivationStore::from_rollback_image(
                image.dialogue_activations,
                &owner,
                &image.expected_deferred_children,
            )?,
            dialogue_effect_callback_activations: image.dialogue_effect_callback_activations,
            run_child_next: image.run_child_next,
            pure_i64_batch_inputs: image.pure_i64_batch_inputs,
            pure_i64_batch_outputs: image.pure_i64_batch_outputs,
            pure_helper_i64_call_shapes: image.pure_helper_i64_call_shapes,
            audio_epoch: image.audio_epoch,
            next_audio_sequence: image.next_audio_sequence,
            next_host_call_sequence: image.next_host_call_sequence,
        };
        restored.validate_scheduled_scope_state()?;
        Ok(restored)
    }

    fn validate_scheduled_scope_state(&self) -> Result<(), String> {
        if self.next_scheduled_scope_sequence == 0 {
            return Err("native control scope allocator is zero".into());
        }
        let mut opened = BTreeSet::new();
        let mut closed = BTreeSet::new();
        for fiber in std::iter::once(&self.fiber).chain(self.child_fibers.iter()) {
            let valid = |token: crate::scope::RuntimeScheduledScopeToken| {
                token.belongs_to(fiber.execution, fiber.persistent_id)
                    && token.ordinal().get() < self.next_scheduled_scope_sequence
            };
            let mut pending = Vec::new();
            pending.extend(fiber.pending_ops.iter());
            for frame in &fiber.control_stack {
                match &frame.kind {
                    FlowControlStackEntryKind::Scope {
                        origin: crate::scope::RuntimeScopeFrameOrigin::Scheduled(token),
                        ..
                    } => {
                        if !valid(*token) || !opened.insert(*token) {
                            return Err(
                                "native scope frame has a foreign, repeated, or unallocated token"
                                    .into(),
                            );
                        }
                    }
                    FlowControlStackEntryKind::FunctionCall(frame) => {
                        pending.extend(frame.caller_pending_ops.iter())
                    }
                    FlowControlStackEntryKind::FormatAttempt(frame) => {
                        if let Some(active) = &frame.active {
                            pending.extend(active.caller_pending_ops.iter())
                        }
                    }
                    _ => {}
                }
            }
            for op in &pending {
                match op {
                    FlowOp::EnterScheduledScope { identity, token } => {
                        if token.kind() == crate::scope::RuntimeScopeFrameKind::Control
                            && !matches!(identity, crate::scope::RuntimeScopeIdentity::Anonymous)
                        {
                            return Err(
                                "native generated scope carries an authored namespace".into()
                            );
                        }
                        if !valid(*token) || !opened.insert(*token) {
                            return Err("native pending scope open has a foreign, repeated, or unallocated token".into());
                        }
                    }
                    FlowOp::ExitScheduledScope { token } => {
                        if !valid(*token) || !closed.insert(*token) {
                            return Err("native pending scope close has a foreign, repeated, or unallocated token".into());
                        }
                    }
                    _ => {}
                }
            }
        }
        if opened != closed {
            return Err(
                "native scope frames and queued exact close markers are not bijective".into(),
            );
        }
        Ok(())
    }

    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    #[must_use]
    pub fn restartable_dispatches(&self) -> Vec<crate::task::RuntimeNeedProducerDispatch> {
        self.need_producers.restartable_dispatches()
    }

    #[must_use]
    pub fn quiescence_blocking_needs(&self) -> Vec<crate::task::NeedId> {
        self.need_producers.quiescence_blocking_needs()
    }

    #[must_use]
    pub fn need_producer_generation_for_task(&self, task: &TaskId) -> Option<GenerationId> {
        self.need_producers.generation_for_task(task)
    }

    pub(crate) fn rebind_generation(&mut self, generation: GenerationId) {
        self.generation = generation;
    }

    /// Retains the exact executable type authority for an asynchronous host result.
    pub fn program_plan(&self) -> Arc<RuntimePlan> {
        Arc::clone(&self.plan)
    }

    pub(super) fn allocate_dialogue_activation(
        &mut self,
        content: crate::runtime_id::RuntimeDialogueContentPlanId,
    ) -> Result<DialogueActivationId, crate::line_task::LineRuntimeError> {
        let artifact = self
            .plan
            .artifact()
            .ok_or(crate::line_task::LineRuntimeError::UnboundArtifact)?;
        let key = (self.fiber.persistent_id, content);
        let occurrence = self.dialogue_occurrences.get(&key).copied().unwrap_or(0);
        let next = occurrence
            .checked_add(1)
            .ok_or(crate::line_task::LineRuntimeError::DialogueOccurrenceOverflow)?;
        let activation =
            DialogueActivationId::new(artifact, self.fiber.persistent_id, content, occurrence);
        self.dialogue_occurrences.insert(key, next);
        Ok(activation)
    }

    /// Creates an engine without implicitly selecting a flow.
    ///
    /// Flow-bearing plans remain dormant until [`Self::start_flow`] or
    /// [`Self::start_entry`] is called. Plans that contain only line tasks,
    /// streams remain directly executable.
    pub fn new(plan: RuntimePlan) -> Self {
        Self::new_with_generation(plan, GenerationId::new(0))
    }

    /// Creates an engine bound to one host-owned generation slot. Runtime
    /// owners that can hot-swap plans must supply the active slot so stale
    /// task events cannot publish into a later generation.
    pub fn new_with_generation(
        plan: impl Into<Arc<RuntimePlan>>,
        generation: GenerationId,
    ) -> Self {
        Self::new_with_shared_plan(plan.into(), generation)
    }

    fn new_with_shared_plan(plan: Arc<RuntimePlan>, generation: GenerationId) -> Self {
        let main_started = plan.flows.is_empty();
        let status = if plan.is_empty() {
            FlowFiberStatus::Done(FlowExit::Done)
        } else {
            FlowFiberStatus::Running
        };
        let stream_states = plan
            .stream_plans
            .iter()
            .map(|plan| {
                (
                    plan.id().clone(),
                    StreamRuntimeState::new(plan.id().clone()),
                )
            })
            .collect();
        let pure_helper_i64_call_shapes = pure_helper_i64_call_shapes(&plan);
        Self {
            evaluation_stats: crate::pure::PureFunctionStats::default(),
            invocation_result: None,
            plan,
            format_context: crate::value::RuntimeFormatContext::default(),
            generation,
            need_producers: NeedProducerRegistry::default(),
            task_request_quota_remaining: usize::MAX,
            scalar_step_charge: None,
            need_publications: BTreeMap::new(),
            latest_need_publications: BTreeMap::new(),
            need_publication_frontiers: BTreeMap::new(),
            main_started,
            root: None,
            fiber: FlowFiber {
                line_cursor: 0,
                cursor: None,
                pending_ops: VecDeque::new(),
                control_stack: Vec::new(),
                await_observer: None,
                root_cleanups: Vec::new(),
                env: RuntimeEnv::default(),
                observations: RuntimeObservationState::default(),
                stream_states,
                selected_dialogue_result: None,
                id: FlowFiberId::default(),
                persistent_id: RuntimePersistentFiberId::from_allocated(1),
                execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                    std::num::NonZeroU64::MIN,
                ),
                owner: FlowFiberOwner::Executor,
                status,
            },
            child_fibers: VecDeque::new(),
            next_fiber_id: 1,
            next_scheduled_scope_sequence: 1,
            dialogue_occurrences: BTreeMap::new(),
            dialogue_activations: dialogue::DialogueActivationStore::default(),
            dialogue_effect_callback_activations: BTreeSet::new(),
            run_child_next: false,
            pure_i64_batch_inputs: Vec::new(),
            pure_i64_batch_outputs: Vec::new(),
            pure_helper_i64_call_shapes,
            audio_epoch: 0,
            next_audio_sequence: 0,
            next_host_call_sequence: 0,
        }
    }

    /// Selects the session locale for subsequent native `fmt` evaluations.
    pub fn set_format_context(&mut self, context: crate::value::RuntimeFormatContext) {
        self.format_context = context;
    }

    /// Actual expression evaluation counters from this Engine's interpreter.
    /// These counters are transactionally restored with its owning state.
    pub(crate) fn evaluation_stats(&self) -> &crate::pure::PureFunctionStats {
        &self.evaluation_stats
    }

    /// Transfers a detached completed value once. A resource-bearing value
    /// remains in this executor until its ledger can be transferred with it.
    pub fn take_program_result(
        &mut self,
    ) -> Result<
        Option<(
            arcweft_id::runtime_program::RuntimePureProgramId,
            RuntimeValue,
        )>,
        crate::value::ownership::RuntimeDetachedValueError,
    > {
        if let Some((_, value)) = &self.invocation_result {
            value.validate_detached_custody()?;
        }
        let Some((NativeInvocationRoot::Program(_), _)) = self.invocation_result.as_ref() else {
            return Ok(None);
        };
        Ok(self.invocation_result.take().map(|(root, value)| {
            let NativeInvocationRoot::Program(program) = root else {
                unreachable!("the borrowed preflight selected this program result")
            };
            (program, value)
        }))
    }

    #[must_use]
    pub const fn format_context(&self) -> &crate::value::RuntimeFormatContext {
        &self.format_context
    }

    /// Creates an engine and selects the requested flow exactly.
    pub fn for_flow(plan: RuntimePlan, flow: &FlowRuntimeId) -> Result<Self, EngineStartError> {
        Self::for_flow_with_generation(plan, flow, GenerationId::new(0))
    }

    /// Creates an engine, generation-bound, and selects the requested flow.
    pub fn for_flow_with_generation(
        plan: RuntimePlan,
        flow: &FlowRuntimeId,
        generation: GenerationId,
    ) -> Result<Self, EngineStartError> {
        let invocation = plan
            .seal_flow_invocation(flow.clone(), [])
            .map_err(|error| EngineStartError::InvalidFlowInvocation {
                message: error.to_string(),
            })?;
        Self::for_flow_invocation_with_generation(invocation, generation)
    }

    /// Creates an engine from one complete plan-owned Flow invocation.
    pub fn for_flow_invocation(
        invocation: crate::plan::RuntimeFlowInvocation,
    ) -> Result<Self, EngineStartError> {
        Self::for_flow_invocation_with_generation(invocation, GenerationId::new(0))
    }

    /// Creates an engine from one complete plan-owned Flow invocation and
    /// the host generation that owns any emitted Need producer tasks.
    pub fn for_flow_invocation_with_generation(
        invocation: crate::plan::RuntimeFlowInvocation,
        generation: GenerationId,
    ) -> Result<Self, EngineStartError> {
        Self::for_flow_invocation_with_need_context(
            invocation,
            generation,
            NeedProducerRegistry::default(),
        )
    }

    /// Transfers Need producer custody together with coordinate-addressed inputs.
    pub fn for_flow_invocation_with_need_context(
        invocation: crate::plan::RuntimeFlowInvocation,
        generation: GenerationId,
        need_producers: NeedProducerRegistry,
    ) -> Result<Self, EngineStartError> {
        let (plan, flow, bindings) = invocation.into_parts();
        for binding in &bindings {
            crate::value::visit_runtime_value_graph(&binding.value, |value| {
                if let RuntimeValue::NeedHandle(handle) = value
                    && need_producers.launch_for_handle(handle).is_none()
                {
                    return Err(EngineStartError::InvalidFlowInvocation {
                        message: "Flow Need input lacks its complete accepted producer context"
                            .to_owned(),
                    });
                }
                Ok(())
            })?;
        }
        let mut engine = Self::new_with_generation(plan, generation);
        engine.need_producers = need_producers;
        engine.start_flow_cursor(&flow)?;
        let admitted = engine
            .validate_current_flow_parameter_bindings(bindings.iter())
            .map_err(|error| EngineStartError::InvalidFlowInvocation {
                message: error.to_string(),
            })?
            .into_iter()
            .zip(bindings)
            .map(|((_, local), binding)| RuntimeLocalBinding {
                local,
                value: binding.value,
            });
        engine.fiber.env.bind_all_root(admitted);
        Ok(engine)
    }

    /// Creates an engine and selects the requested entry exactly.
    pub fn for_entry(plan: RuntimePlan, entry: &EntryRuntimeId) -> Result<Self, EngineStartError> {
        Self::for_entry_with_generation(plan, entry, GenerationId::new(0))
    }

    /// Creates a generation-bound engine and selects the requested entry.
    pub fn for_entry_with_generation(
        plan: RuntimePlan,
        entry: &EntryRuntimeId,
        generation: GenerationId,
    ) -> Result<Self, EngineStartError> {
        let mut engine = Self::new_with_generation(plan, generation);
        engine.start_entry(entry)?;
        Ok(engine)
    }

    /// Selects one flow before the first flow execution step.
    pub fn start_flow(&mut self, flow: &FlowRuntimeId) -> Result<(), EngineStartError> {
        let schema = self.plan.flows.schema(flow).ok_or_else(|| {
            EngineStartError::InvalidFlowInvocation {
                message: format!("Flow `{flow}` has no invocation schema"),
            }
        })?;
        if !schema.parameters.is_empty() {
            return Err(EngineStartError::InvalidFlowInvocation {
                message: format!(
                    "Flow `{flow}` requires an explicit coordinate-addressed invocation"
                ),
            });
        }
        self.start_flow_cursor(flow)
    }

    fn start_flow_cursor(&mut self, flow: &FlowRuntimeId) -> Result<(), EngineStartError> {
        if self.main_started {
            return Err(EngineStartError::AlreadyStarted);
        }
        let flow_index = self
            .flow_index(flow)
            .ok_or_else(|| EngineStartError::MissingFlow {
                flow: flow.canonical_label(),
            })?;
        self.fiber.cursor = Some(FlowCursor {
            flow_index,
            op_index: 0,
        });
        self.fiber.status = FlowFiberStatus::Running;
        self.main_started = true;
        Ok(())
    }

    /// Selects the single flow named by an exact entry identity.
    pub fn start_entry(&mut self, entry: &EntryRuntimeId) -> Result<(), EngineStartError> {
        if self.main_started {
            return Err(EngineStartError::AlreadyStarted);
        }
        let target = self
            .plan
            .entries
            .iter()
            .find(|candidate| candidate.id == *entry)
            .map(|candidate| candidate.target.clone())
            .ok_or_else(|| EngineStartError::MissingEntry {
                entry: entry.canonical_label(),
            })?;
        let flow = match target {
            RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => flow,
            RuntimeEntryTarget::Routes(_) => {
                return Err(EngineStartError::EntryDoesNotSelectFlow {
                    entry: entry.canonical_label(),
                });
            }
        };
        if matches!(
            self.plan
                .entries
                .iter()
                .find(|candidate| candidate.id == *entry)
                .map(|candidate| &candidate.roles),
            Some(crate::entry::RuntimeEntryRoles::Stateful(_))
        ) {
            let contract =
                RootStartupContract::from_runtime_plan(&self.plan, entry).map_err(|error| {
                    EngineStartError::InvalidRootStartup {
                        entry: entry.canonical_label(),
                        message: error.to_string(),
                    }
                })?;
            let mut evaluator = StructuredRootEvaluator::new(&self.plan);
            let startup = RootRuntime::start(
                contract,
                &mut evaluator,
                crate::program_types::RuntimeProgramTypes::Plan(&self.plan),
            )
            .map_err(|error| EngineStartError::InvalidRootStartup {
                entry: entry.canonical_label(),
                message: error.to_string(),
            })?;
            let flow_index = self.flow_index(&startup.initial_flow).ok_or_else(|| {
                EngineStartError::MissingFlow {
                    flow: startup.initial_flow.canonical_label(),
                }
            })?;
            self.fiber.cursor = Some(FlowCursor {
                flow_index,
                op_index: 0,
            });
            self.fiber.status = FlowFiberStatus::Running;
            let initial_state_local = self
                .validate_current_flow_parameter_bindings(std::iter::once(
                    &startup.initial_state_binding,
                ))
                .and_then(|mut bindings| {
                    bindings.pop().map(|(_, local)| local).ok_or_else(|| {
                        RuntimeEvalError::UnknownFlowBinding {
                            flow: startup.initial_flow.canonical_label(),
                            binding: format!(
                                "#{}",
                                startup.initial_state_binding.parameter.position()
                            ),
                        }
                    })
                })
                .map_err(|error| EngineStartError::InvalidRootStartup {
                    entry: entry.canonical_label(),
                    message: error.to_string(),
                })?;
            if !startup
                .initial_state_binding
                .value
                .ownership()
                .permits_copy()
            {
                return Err(EngineStartError::InvalidRootStartup {
                    entry: entry.canonical_label(),
                    message: "root startup state must be unrestricted".to_owned(),
                });
            }
            self.fiber.env.set_root(
                initial_state_local,
                startup.initial_state_binding.value.clone(),
            );
            self.root = Some(startup.root);
            self.main_started = true;
            return Ok(());
        }
        self.start_flow(&flow)
    }

    #[must_use]
    pub const fn root(&self) -> Option<&RootRuntime> {
        self.root.as_ref()
    }

    pub fn acknowledge_root_commands(
        &mut self,
        accepted: &[RuntimeCommandEnvelope],
    ) -> Result<(), RootRuntimeError> {
        match self.root.as_mut() {
            Some(root) => root.acknowledge_published_commands(accepted),
            None if accepted.is_empty() => Ok(()),
            None => Err(RootRuntimeError::CommandAcknowledgementMismatch),
        }
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.fiber
    }

    pub(super) fn flow_at_cursor(&self, cursor: &FlowCursor) -> Option<&RuntimeFlow> {
        self.plan.flows.get(cursor.flow_index)
    }

    pub(super) fn flow_index(&self, flow: &FlowRuntimeId) -> Option<usize> {
        self.plan.flows.position(flow)
    }

    fn validate_current_flow_parameter_bindings<'a>(
        &self,
        bindings: impl IntoIterator<Item = &'a RuntimeFlowParameterBinding>,
    ) -> Result<
        Vec<(
            crate::entry::FlowParameterCoordinate,
            crate::runtime_id::RuntimeLocalDeclarationId,
        )>,
        RuntimeEvalError,
    > {
        let mut bindings = bindings.into_iter().peekable();
        let Some(first) = bindings.peek() else {
            return Ok(Vec::new());
        };
        let binding_label = format!("#{}", first.parameter.position());
        let Some(cursor) = self.fiber.cursor.as_ref() else {
            return Err(RuntimeEvalError::MissingFlowBindingTarget {
                flow: "<none>".to_owned(),
                binding: binding_label,
            });
        };
        let Some(flow) = self.plan.flows.get(cursor.flow_index) else {
            return Err(RuntimeEvalError::MissingFlowBindingTarget {
                flow: format!("#{}", cursor.flow_index),
                binding: binding_label,
            });
        };
        let flow_label = flow.id.canonical_label();
        let Some(schema) = self.plan.flows.schema(&flow.id) else {
            return Err(RuntimeEvalError::MissingFlowBindingTarget {
                flow: flow_label,
                binding: binding_label,
            });
        };

        let mut admitted = Vec::new();
        let mut unique = BTreeSet::new();
        for binding in bindings {
            if !unique.insert(binding.parameter) {
                return Err(RuntimeEvalError::DuplicateFlowParameterBinding {
                    flow: flow_label,
                    parameter: binding.parameter,
                });
            }
            let position = binding.parameter.index().map_err(|_| {
                RuntimeEvalError::InvalidFlowParameterCoordinate {
                    flow: flow_label.clone(),
                    parameter: binding.parameter,
                }
            })?;
            let Some(parameter) = schema
                .parameters
                .get(position)
                .filter(|parameter| parameter.coordinate == binding.parameter)
            else {
                return Err(RuntimeEvalError::UnknownFlowParameterBinding {
                    flow: flow_label,
                    parameter: binding.parameter,
                });
            };
            let local = flow.params.get(position).copied().ok_or_else(|| {
                RuntimeEvalError::MissingFlowParameterLocal {
                    flow: flow_label.clone(),
                    position,
                }
            })?;
            let declaration = self
                .plan
                .local_declarations
                .get(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?;
            if !self
                .plan
                .value_matches_type(declaration.ty(), &binding.value)?
            {
                return Err(RuntimeEvalError::FlowParameterBindingType {
                    flow: flow_label,
                    parameter: parameter.coordinate,
                    local,
                    expected: declaration.ty(),
                });
            }
            admitted.push((binding.parameter, local));
        }
        Ok(admitted)
    }

    pub fn child_fiber_count(&self) -> usize {
        self.child_fibers.len()
    }

    pub(crate) fn admit_host_call(
        &mut self,
        start: crate::step::RuntimeHostCallStart,
    ) -> Result<crate::step::RuntimeHostCallRequest, crate::task::NeedProducerAdmissionError> {
        crate::step::RuntimeHostCallRequest::admit_start(
            start,
            self.generation,
            &mut self.need_producers,
        )
    }

    fn latch_need_publications(
        &mut self,
        states: Vec<crate::task::RuntimeNeedState>,
        events: Vec<TaskEvent>,
        output: &mut RuntimeStepOutput,
    ) -> Vec<TaskEvent> {
        for state in states {
            if self
                .need_producers
                .launch_for_correlation(&state.correlation)
                .is_some()
            {
                match self.need_producers.publish_need_state(&state) {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(error) => {
                        let message = format!("invalid Need publication: {error}");
                        self.fiber.status = FlowFiberStatus::Failed(message.clone());
                        output.diagnostics.push(RuntimeDiagnostic::categorized(
                            RuntimeDiagnosticCategory::Host,
                            message,
                        ));
                        return events;
                    }
                }
            }
            let (correlation, cursor, state) = state.into_parts();
            let Some(cursor) = cursor else {
                continue;
            };
            if !self.enqueue_need_publication(
                RuntimeNeedPublication::State {
                    correlation,
                    state,
                    cursor,
                },
                output,
            ) {
                return events;
            }
        }
        let mut remaining = Vec::new();
        for event in events {
            let Some(publication) = self.need_producers.publication_for_task_event(&event) else {
                remaining.push(event);
                continue;
            };
            match self.need_producers.publish_task_event_owned(event) {
                Ok(NeedProducerOwnedTaskEventDisposition::Published) => {}
                Ok(NeedProducerOwnedTaskEventDisposition::Duplicate(_)) => continue,
                Ok(NeedProducerOwnedTaskEventDisposition::NotLocal(event)) => {
                    remaining.push(event);
                    continue;
                }
                Err(error) => {
                    let (reason, _rejected_event) = error.into_parts();
                    let message = format!("invalid Need task publication: {reason}");
                    self.fiber.status = FlowFiberStatus::Failed(message.clone());
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Host,
                        message,
                    ));
                    return remaining;
                }
            }
            if !self.enqueue_need_publication(publication, output) {
                return remaining;
            }
        }
        remaining
    }

    fn enqueue_need_publication(
        &mut self,
        publication: RuntimeNeedPublication,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let need = publication.correlation();
        let cursor = publication.cursor();
        let image = match publication.inert_rollback_image(&crate::task::RuntimeProgramOwner::Plan(
            Arc::clone(&self.plan),
        )) {
            Ok(image) => image,
            Err(message) => {
                self.fiber.status = FlowFiberStatus::Failed(message.clone());
                output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Host,
                    message,
                ));
                return false;
            }
        };
        if let Some(previous) = self.need_publication_frontiers.get(&need).copied() {
            match previous.compare_same_source(cursor) {
                Some(std::cmp::Ordering::Greater) => return true,
                Some(std::cmp::Ordering::Equal) => {
                    if self.latest_need_publications.get(&need) == Some(&image) {
                        return true;
                    }
                    let message = format!(
                        "Need {need:?} received conflicting publications at {:?}",
                        cursor
                    );
                    self.fiber.status = FlowFiberStatus::Failed(message.clone());
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Host,
                        message,
                    ));
                    return false;
                }
                Some(std::cmp::Ordering::Less) => {}
                None => {
                    let message = format!(
                        "Need {need:?} changed publication authority between external state and a local task"
                    );
                    self.fiber.status = FlowFiberStatus::Failed(message.clone());
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Host,
                        message,
                    ));
                    return false;
                }
            }
        }
        self.need_publication_frontiers.insert(need.clone(), cursor);
        self.latest_need_publications.insert(need.clone(), image);
        self.need_publications
            .entry(need)
            .or_default()
            .push_back(publication);
        true
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
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        let mut output = RuntimeStepOutput::default();
        self.task_request_quota_remaining = options.max_new_task_requests;
        let mut executed_ops = 0;
        let pure_stats_before = pure_backend.stats();
        let pending_ops_before = self.pending_ops_len();
        let root_events_in = input.root_events.len();
        if let Some(error) = input
            .task_events
            .iter()
            .find_map(|event| event.inspect_host_ready_ownership().err())
            .or_else(|| {
                input
                    .need_states
                    .iter()
                    .find_map(|state| state.inspect_host_ready_ownership().err())
            })
            .or_else(|| {
                input
                    .host_call_results
                    .iter()
                    .find_map(|result| result.inspect_host_payload_ownership().err())
            })
        {
            let message = error.to_string();
            self.fiber.status = FlowFiberStatus::Failed(message.clone());
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Host,
                message,
            ));
            let stats = RuntimeStepStats {
                pending_ops_before,
                pending_ops_after: self.pending_ops_len(),
                child_fibers: self.child_fibers.len(),
                root_events_in,
                task_events_in: input.task_events.len(),
                diagnostics: output.diagnostics.len(),
                ..RuntimeStepStats::default()
            };
            return self.step_result(output, options, stats);
        }
        let deferred_root_events = std::mem::take(&mut input.deferred_root_events);
        let need_states = normalize_runtime_need_states(std::mem::take(&mut input.need_states));
        let need_states_in = need_states.len();
        output
            .requests
            .root_events_next_step
            .extend(deferred_root_events);
        let dialogue_content_events = std::mem::take(&mut input.dialogue_content_events);
        let dialogue_advances = std::mem::take(&mut input.dialogue_advances);
        let line_outcomes = std::mem::take(&mut input.line_outcomes);
        let dialogue_ingress = match self.dialogue_activations.latch_step_input(
            input.dt,
            &dialogue_content_events,
            &dialogue_advances,
            &line_outcomes,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                let activation = error.activation().cloned();
                let source = error.into_source();
                if let Some(activation) = activation.filter(|activation| {
                    matches!(
                        &self.fiber.status,
                        FlowFiberStatus::Dialogue(current) if current.activation() == activation
                    )
                }) {
                    match self.begin_dialogue_activation_transaction(&activation) {
                        Ok(transaction) => {
                            self.begin_dialogue_failure(transaction, source.into(), &mut output);
                        }
                        Err(begin_error) => self.fail_eval(begin_error, &mut output),
                    }
                } else {
                    let message = source.to_string();
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Input,
                        message.clone(),
                    ));
                    self.fiber.status = FlowFiberStatus::Failed(message);
                }
                let stats = RuntimeStepStats {
                    executed_ops,
                    pending_ops_before,
                    pending_ops_after: self.pending_ops_len(),
                    child_fibers: self.child_fibers.len(),
                    pure: pure_backend.stats().saturating_delta(pure_stats_before),
                    root_events_in,
                    root_transitions: output.root_transitions.len(),
                    root_commands: output.root_commands.len(),
                    root_events_deferred: output.requests.root_events_next_step.len(),
                    diagnostics: output.diagnostics.len(),
                    ..RuntimeStepStats::default()
                };
                return self.step_result(output, options, stats);
            }
        };
        for diagnostic in dialogue_ingress.into_diagnostics() {
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Host,
                diagnostic.to_string(),
            ));
        }
        if !self.run_root_phase(std::mem::take(&mut input.root_events), &mut output) {
            let stats = RuntimeStepStats {
                executed_ops,
                pending_ops_before,
                pending_ops_after: self.pending_ops_len(),
                child_fibers: self.child_fibers.len(),
                pure: pure_backend.stats().saturating_delta(pure_stats_before),
                root_events_in,
                root_transitions: output.root_transitions.len(),
                root_commands: output.root_commands.len(),
                diagnostics: output.diagnostics.len(),
                ..RuntimeStepStats::default()
            };
            return self.step_result(output, options, stats);
        }
        let events = normalize_task_events(std::mem::take(&mut input.task_events));
        let task_events_in = events.len();
        output.diagnostics.extend(events.iter().map(|event| {
            RuntimeDiagnostic::new(format!(
                "task {} sequence {} delivered",
                event.correlation.task_id, event.cursor.sequence.0
            ))
        }));
        let events = self.latch_need_publications(need_states, events, &mut output);
        self.step_stream_plans(&mut output, pure_backend);

        while executed_ops < options.budget.max_ops && self.can_attempt_runtime_op() {
            let consumed = self
                .try_step_numeric_map_span(
                    options.budget.max_ops - executed_ops,
                    options.mode,
                    &mut output,
                    pure_backend,
                )
                .unwrap_or_else(|| {
                    let (_, control_ops) = self.with_scalar_operation_budget(
                        options.mode,
                        options.budget.max_ops - executed_ops - 1,
                        |engine| {
                            engine.step_runtime_op(&mut input, &events, &mut output, pure_backend)
                        },
                    );
                    1 + control_ops
                });
            executed_ops += consumed;
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
        self.step_result(output, options, stats)
    }

    /// Carries the owning driver's remaining budget through the existing
    /// operation transaction. A body completion charges only after its actual
    /// physical result is produced; declines leave the original frame intact.
    fn with_scalar_operation_budget<R>(
        &mut self,
        mode: RuntimeStepMode,
        remaining: usize,
        action: impl FnOnce(&mut Self) -> R,
    ) -> (R, usize) {
        debug_assert!(self.scalar_step_charge.is_none());
        self.scalar_step_charge = Some(NativeScalarStepCharge {
            mode,
            fiber: self.fiber.id,
            remaining,
            charged: 0,
        });
        let result = action(self);
        let charged = self
            .scalar_step_charge
            .take()
            .map_or(0, |charge| charge.charged);
        (result, charged)
    }
    fn run_root_phase(
        &mut self,
        events: Vec<RootEventInput>,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let Some(root) = self.root.as_mut() else {
            if events.is_empty() {
                return true;
            }
            let message = "non-stateful runtime entry cannot accept root events".to_owned();
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Input,
                message,
            ));
            return false;
        };
        let result = {
            let mut evaluator = StructuredRootEvaluator::new(&self.plan);
            root.step(
                events,
                &mut evaluator,
                crate::program_types::RuntimeProgramTypes::Plan(&self.plan),
            )
        };
        match result {
            Ok(result) => {
                let failed = result.failed;
                output.root_transitions.extend(result.outcomes);
                output.root_commands.extend(result.commands);
                if failed {
                    let message = root
                        .failure()
                        .map_or_else(|| "root reducer trapped".to_owned(), ToString::to_string);
                    self.fiber.status = FlowFiberStatus::Failed(message);
                    false
                } else {
                    true
                }
            }
            Err(error) => {
                let step_input_rejection = matches!(
                    &error,
                    RootRuntimeError::InvalidEvent(_)
                        | RootRuntimeError::EventQueueLimit { .. }
                        | RootRuntimeError::TransitionSequenceExhausted
                );
                let category = if step_input_rejection {
                    RuntimeDiagnosticCategory::Input
                } else {
                    RuntimeDiagnosticCategory::Internal
                };
                let message = error.to_string();
                output
                    .diagnostics
                    .push(RuntimeDiagnostic::categorized(category, message.clone()));
                if !step_input_rejection {
                    self.fiber.status = FlowFiberStatus::Failed(message);
                }
                false
            }
        }
    }

    fn can_attempt_runtime_op(&self) -> bool {
        self.main_fiber_can_attempt_runtime_op() || self.has_executor_work()
    }

    fn step_runtime_op(
        &mut self,
        input: &mut RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
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
        self.latch_active_await_observer_publications();
        if self.resume_suspended(input, events, output, pure_backend) {
            return;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return;
        }
        if self.fiber.cursor.is_some()
            || !self.fiber.pending_ops.is_empty()
            || self.has_active_project_call()
        {
            self.step_main_flow_transaction(output, pure_backend);
        } else {
            self.step_line_only(input, output, pure_backend);
        }
    }

    fn step_main_flow_transaction(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        self.with_main_flow_transaction(
            output,
            pure_backend,
            |candidate, staged_output, backend, drop_policy| {
                candidate.step_flow(staged_output, backend, drop_policy);
                1
            },
        );
    }
    fn with_main_flow_transaction<B: RuntimeCallBackend>(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut B,
        execute: impl FnOnce(
            &mut Self,
            &mut RuntimeStepOutput,
            &mut B,
            &mut Option<crate::effect::RuntimeDropPolicy>,
        ) -> usize,
    ) -> usize {
        let before = match self.main_fiber_line_handle_owners() {
            Ok(owners) => owners,
            Err(error) => {
                self.fail_eval(error, output);
                return 1;
            }
        };
        let image = match self.inert_rollback_image() {
            Ok(image) => image,
            Err(error) => {
                self.fail_eval(error, output);
                return 1;
            }
        };
        let mut candidate = std::mem::replace(
            self,
            Self::new_with_shared_plan(Arc::clone(&self.plan), self.generation),
        );
        let mut staged_output = RuntimeStepOutput::default();
        let mut drop_policy = None;
        let executed = execute(
            &mut candidate,
            &mut staged_output,
            pure_backend,
            &mut drop_policy,
        );
        let mut drops = candidate.fiber.env.take_assignment_discard_authorization();
        if let Err(error) = drops.set_boundary(drop_policy) {
            drop(candidate);
            *self = Self::from_rollback_image(image)
                .expect("a native Engine rollback image reconstructs its admitted owner");
            self.fail_eval(error, output);
            return 1;
        }
        let after = match candidate.main_fiber_line_handle_owners() {
            Ok(owners) => owners,
            Err(error) => {
                drop(candidate);
                *self = Self::from_rollback_image(image)
                    .expect("a native Engine rollback image reconstructs its admitted owner");
                self.fail_eval(error, output);
                return 1;
            }
        };
        let receipt = match candidate.dialogue_activations.reconcile_parent_fiber(
            candidate.fiber.execution,
            &before,
            &after,
            &drops,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                drop(candidate);
                *self = Self::from_rollback_image(image)
                    .expect("a native Engine rollback image reconstructs its admitted owner");
                self.fail_eval(error, output);
                return 1;
            }
        };
        staged_output
            .requests
            .line_commands
            .extend(receipt.into_commands());
        *self = candidate;
        output.merge(staged_output);
        executed
    }

    fn main_fiber_can_attempt_runtime_op(&self) -> bool {
        self.main_started
            && !matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            )
    }

    /// Joined work controls parent flow completion. Detached line work remains
    /// executor work but is intentionally excluded from this local join.
    pub(super) fn has_joined_work(&self) -> bool {
        self.child_fibers
            .iter()
            .any(|child| child.owner.has_joined_work())
    }

    /// Executor work controls scheduling. It includes detached line fibers so
    /// their scopes are still unwound by their owning executor.
    pub(super) fn has_executor_work(&self) -> bool {
        !self.child_fibers.is_empty()
    }

    /// Requests cancellation without deleting a queued child. Cancel-and-join
    /// transitions the owner into Closing; the scheduler subsequently enters
    /// that fiber and performs its lexical cleanup before reporting completion.
    pub(super) fn request_line_task_cancellation(&mut self) {
        for child in &mut self.child_fibers {
            let FlowFiberOwner::LineTask(owner) = &mut child.owner else {
                continue;
            };
            match owner.cancel_policy {
                ChildCancelPolicy::Finish => {}
                // Builder admission rejects Detach until it has a proved
                // ownership-transfer target. A malformed in-memory plan must
                // fail closed rather than silently changing ownership.
                ChildCancelPolicy::CancelAndJoin | ChildCancelPolicy::Detach => {
                    owner.closing = true;
                }
            }
        }
    }

    fn pending_ops_len(&self) -> usize {
        self.fiber.pending_ops.len()
            + self
                .child_fibers
                .iter()
                .map(|fiber| fiber.pending_ops.len())
                .sum::<usize>()
    }

    fn prepare_child_fiber(
        &self,
        body: Vec<FlowOp>,
        captures: &[crate::runtime_id::RuntimeLocalDeclarationId],
    ) -> Result<(FlowFiber, u64), RuntimeEvalError> {
        let env = self.fiber.env.try_capture_unrestricted(captures)?;
        let ordinal = self.next_fiber_id;
        let next = ordinal
            .checked_add(1)
            .and_then(std::num::NonZeroU64::new)
            .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
        let mut pending_ops = VecDeque::with_capacity(body.len().saturating_add(2));
        if !body.is_empty() {
            pending_ops.push_front(FlowOp::ExitScope);
            for op in body.into_iter().rev() {
                pending_ops.push_front(op);
            }
            pending_ops.push_front(FlowOp::EnterScope {
                identity: crate::scope::RuntimeScopeIdentity::Anonymous,
            });
        }
        Ok((
            FlowFiber {
                line_cursor: 0,
                cursor: None,
                pending_ops,
                control_stack: Vec::new(),
                await_observer: None,
                root_cleanups: Vec::new(),
                env,
                observations: RuntimeObservationState::default(),
                stream_states: BTreeMap::new(),
                selected_dialogue_result: None,
                id: FlowFiberId(ordinal),
                persistent_id: RuntimePersistentFiberId::from_allocated(next.get()),
                execution: crate::runtime_id::ExecutionInstanceId::from_allocated(next),
                owner: FlowFiberOwner::Executor,
                status: FlowFiberStatus::Running,
            },
            next.get(),
        ))
    }
    fn inspect_dialogue_effect_callback(
        &self,
        activation: &DialogueActivationId,
        site: crate::runtime_id::RuntimeDialogueEffectSiteId,
        callback: &RuntimeCallableValue,
        ordinal: u64,
    ) -> Result<NativePreparedDialogueEffectCallback, RuntimeEvalError> {
        callback.validate_for_owner(&crate::task::RuntimeProgramOwner::Plan(Arc::clone(
            &self.plan,
        )))?;
        if !callback.is_structured_executable_callback() {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "callable".to_owned(),
                reason: "dialogue reveal requires an executable zero-argument Unit callback"
                    .to_owned(),
            });
        }
        let (invocation, captures, arguments) = callback.inspect_zero_arg_plan_inputs()?;
        let crate::value::RuntimeCallableBodyReference::Plan(function) = invocation.body() else {
            unreachable!("borrowed native callback proof selected a plan body")
        };
        let declaration = self
            .plan
            .validate_function_site_input_refs(function, &captures, &arguments)?;
        if !matches!(declaration.body(), RuntimeFunctionSiteBody::Executable(_)) {
            return Err(
                crate::value::RuntimeCallableValueError::RequiresControlTransfer {
                    state: callback.state(),
                }
                .into(),
            );
        }
        for input in declaration.inputs() {
            let (values, position) = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    (&captures, position as usize)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => {
                    (&arguments, position as usize)
                }
            };
            let value = values.get(position).copied().ok_or(
                crate::value::RuntimeFunctionApplyError::InvalidBoundArgumentPrefix {
                    site: function,
                },
            )?;
            if !crate::pattern::inspect_runtime_pattern_owned(
                &self.plan,
                input.pattern(),
                value,
                declaration.type_instantiation.as_deref(),
            )? {
                return Err(RuntimeEvalError::PatternMismatch(format!(
                    "function site {function} callback input {:?}",
                    input.source()
                )));
            }
        }
        let allocated = ordinal
            .checked_add(1)
            .and_then(std::num::NonZeroU64::new)
            .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
        Ok(NativePreparedDialogueEffectCallback {
            type_instantiation: declaration.type_instantiation.clone(),
            key: crate::runtime_id::RuntimeDialogueEffectCallbackActivationId::new(
                activation.clone(),
                site,
            ),
            ordinal,
            allocated,
            site: function,
            invocation,
        })
    }

    fn commit_dialogue_effect_callback(
        &self,
        callback: RuntimeCallableValue,
        prepared: NativePreparedDialogueEffectCallback,
    ) -> FlowFiber {
        let invocation = callback.commit_zero_arg_invocation(prepared.invocation);
        assert!(matches!(
            invocation.body,
            crate::value::RuntimeCallableBodyReference::Plan(site) if site == prepared.site
        ));
        let site = self
            .plan
            .function_sites()
            .get(prepared.site)
            .expect("prepared callback function site remains present");
        let RuntimeFunctionSiteBody::Executable(executable) = site.body() else {
            unreachable!("prepared callback selected executable body")
        };
        let mut env = RuntimeEnv::default();
        env.push_function_scope(
            prepared.site,
            site.inputs().len(),
            prepared.type_instantiation.clone(),
        );
        let mut captures = invocation
            .captures
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        let mut arguments = invocation
            .arguments
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        for input in site.inputs() {
            let (values, position) = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    (&mut captures, position)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => {
                    (&mut arguments, position)
                }
            };
            let value = values
                .get_mut(position as usize)
                .and_then(Option::take)
                .expect("prepared callback capture source remains present");
            let bindings = crate::pattern::match_runtime_pattern_owned(
                &self.plan,
                input.pattern(),
                value,
                prepared.type_instantiation.as_deref(),
            )
            .expect("prepared callback pattern remains valid")
            .expect("prepared callback pattern remains matched");
            env.bind_all(bindings);
        }
        let mut pending_ops = VecDeque::with_capacity(executable.ops().len().saturating_add(2));
        pending_ops.push_back(FlowOp::EnterScope {
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
        });
        pending_ops.extend(executable.ops().iter().cloned());
        pending_ops.push_back(FlowOp::ExitScope);
        FlowFiber {
            line_cursor: 0,
            cursor: None,
            pending_ops,
            control_stack: Vec::new(),
            await_observer: None,
            root_cleanups: Vec::new(),
            env,
            observations: RuntimeObservationState::default(),
            stream_states: BTreeMap::new(),
            selected_dialogue_result: None,
            id: FlowFiberId(prepared.ordinal),
            persistent_id: RuntimePersistentFiberId::from_allocated(prepared.allocated.get()),
            execution: crate::runtime_id::ExecutionInstanceId::from_allocated(prepared.allocated),
            owner: FlowFiberOwner::Executor,
            status: FlowFiberStatus::Running,
        }
    }

    fn take_dialogue_effect_callbacks(
        callbacks: &mut Box<[RuntimeDialogueContentEffectBinding]>,
        sites: &[crate::runtime_id::RuntimeDialogueEffectSiteId],
    ) -> Result<
        Vec<(
            crate::runtime_id::RuntimeDialogueEffectSiteId,
            RuntimeCallableValue,
        )>,
        RuntimeEvalError,
    > {
        let mut seen = std::collections::BTreeSet::new();
        for site in sites {
            if !seen.insert(*site) || !callbacks.iter().any(|callback| callback.site() == *site) {
                return Err(RuntimeEvalError::Effect(format!(
                    "dialogue effect site {site} has no available stored callback"
                )));
            }
        }
        let mut remaining = std::mem::take(callbacks).into_vec();
        let selected = sites
            .iter()
            .map(|site| {
                let index = remaining
                    .iter()
                    .position(|callback| callback.site() == *site)
                    .expect("all effect callback sites were preflighted before taking owners");
                remaining.remove(index).into_parts()
            })
            .collect();
        *callbacks = remaining.into_boxed_slice();
        Ok(selected)
    }

    pub(super) fn inspect_dialogue_effect_callbacks(
        &self,
        activation: &DialogueActivationId,
        callbacks: &[RuntimeDialogueContentEffectBinding],
        sites: &[crate::runtime_id::RuntimeDialogueEffectSiteId],
        first_ordinal: u64,
    ) -> Result<Vec<NativePreparedDialogueEffectCallback>, RuntimeEvalError> {
        let mut selected = BTreeSet::new();
        let mut keys = self.dialogue_effect_callback_activations.clone();
        let mut ordinal = first_ordinal;
        let mut prepared = Vec::with_capacity(sites.len());
        for site in sites {
            let callback = callbacks
                .iter()
                .find(|callback| callback.site() == *site)
                .ok_or_else(|| {
                    RuntimeEvalError::Effect(format!(
                        "dialogue effect site {site} has no available stored callback"
                    ))
                })?;
            if !selected.insert(*site) {
                return Err(RuntimeEvalError::Effect(format!(
                    "dialogue effect site {site} was selected twice"
                )));
            }
            let proof = self.inspect_dialogue_effect_callback(
                activation,
                *site,
                callback.callback(),
                ordinal,
            )?;
            if !keys.insert(proof.key.clone()) {
                return Err(RuntimeEvalError::Effect(format!(
                    "dialogue effect callback activation was already reserved: {:?}",
                    proof.key
                )));
            }
            ordinal = proof.allocated.get();
            prepared.push(proof);
        }
        Ok(prepared)
    }

    pub(super) fn commit_dialogue_effect_callbacks(
        &self,
        batch: &mut NativeLineTaskExecutionBatch,
        callbacks: Vec<(
            crate::runtime_id::RuntimeDialogueEffectSiteId,
            RuntimeCallableValue,
        )>,
        prepared: Vec<NativePreparedDialogueEffectCallback>,
    ) {
        assert_eq!(callbacks.len(), prepared.len());
        for ((site, callback), proof) in callbacks.into_iter().zip(prepared) {
            assert_eq!(batch.next_fiber_id, proof.ordinal);
            assert_eq!(proof.key.site(), site);
            assert!(
                batch
                    .dialogue_effect_callback_activations
                    .insert(proof.key.clone())
            );
            batch.next_fiber_id = proof.allocated.get();
            let child = self.commit_dialogue_effect_callback(callback, proof);
            batch.child_fibers.push_back(child);
            batch.run_child_next = true;
        }
    }

    pub(super) fn capture_line_task_locals(
        &self,
        group: &LineTaskGroup,
    ) -> Result<Box<[RuntimeLocalBinding]>, crate::line_task::LineRuntimeError> {
        group
            .captures()
            .iter()
            .map(|local| {
                let value = self.fiber.env.get(*local).ok_or(
                    crate::line_task::LineRuntimeError::UnknownOwnedLocal { local: *local },
                )?;
                if !value.ownership().permits_copy() {
                    return Err(crate::line_task::LineRuntimeError::AffineGroupCapture);
                }
                Ok(RuntimeLocalBinding {
                    local: *local,
                    value: value.clone(),
                })
            })
            .collect::<Result<Vec<_>, crate::line_task::LineRuntimeError>>()
            .map(Vec::into_boxed_slice)
    }

    fn inspect_line_task_commands(
        &self,
        transaction: &dialogue::DialogueActivationTransaction,
        activation: &crate::line_task::LineTaskActivation,
        captures: &[RuntimeLocalBinding],
        request_cancellation: bool,
    ) -> Result<NativePreparedLineTaskCommands, dialogue::DialogueExecutionError> {
        let activation_id = transaction.activation();
        let line = transaction.line();
        let mut scheduled = line.begin_scheduled_completion_stage();
        let mut completed = BTreeSet::new();
        for completion in &activation.scheduled_completions {
            line.stage_unstarted_scheduled_completion(&mut scheduled, completion)?;
            completed.insert(completion.token().clone());
        }
        if request_cancellation
            && self.child_fibers.iter().any(|child| {
                matches!(&child.owner,
                    FlowFiberOwner::LineTask(owner)
                        if owner.tag.activation_id() == activation_id
                            && owner.cancel_policy == ChildCancelPolicy::Detach)
            })
        {
            return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation.into());
        }
        let mut packets = BTreeMap::new();
        let mut planned = Vec::new();
        let mut next_fiber_id = self.next_fiber_id;
        for command in &activation.commands {
            match command {
                crate::line_task::LineTaskCommand::Run { tag, policy } => {
                    if let Some(token) = tag.scheduled_token() {
                        if completed.contains(token) || packets.contains_key(token) {
                            return Err(crate::line_task::LineRuntimeError::InvalidScheduledCaptureTransition.into());
                        }
                        packets.insert(token.clone(), line.inspect_scheduled_packet_take(token)?);
                    } else if captures
                        .iter()
                        .any(|capture| !capture.value.ownership().permits_copy())
                    {
                        return Err(crate::line_task::LineRuntimeError::AffineGroupCapture.into());
                    }
                    next_fiber_id = next_fiber_id
                        .checked_add(1)
                        .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
                    planned.push((tag, policy.cancel));
                }
                crate::line_task::LineTaskCommand::Cancel { tag } => {
                    if self.child_fibers.iter().any(|child| {
                        matches!(&child.owner,
                            FlowFiberOwner::LineTask(owner)
                                if owner.tag == *tag
                                    && owner.cancel_policy == ChildCancelPolicy::Detach)
                    }) || planned.iter().any(|(planned_tag, cancel)| {
                        *planned_tag == tag && *cancel == ChildCancelPolicy::Detach
                    }) {
                        return Err(
                            crate::line_task::LineRuntimeError::InvalidActivationOperation.into(),
                        );
                    }
                }
            }
        }
        Ok(NativePreparedLineTaskCommands {
            scheduled,
            packets,
            next_fiber_id,
        })
    }

    pub(super) fn prepare_line_task_commands(
        &self,
        transaction: &mut dialogue::DialogueActivationTransaction,
        group: &LineTaskGroup,
        activation: crate::line_task::LineTaskActivation,
        captures: &[RuntimeLocalBinding],
        request_cancellation: bool,
    ) -> Result<NativeLineTaskExecutionBatch, dialogue::DialogueExecutionError> {
        let NativePreparedLineTaskCommands {
            scheduled,
            mut packets,
            next_fiber_id,
        } = self.inspect_line_task_commands(
            transaction,
            &activation,
            captures,
            request_cancellation,
        )?;
        let activation_id = transaction.activation().clone();
        let mut batch = NativeLineTaskExecutionBatch {
            child_fibers: VecDeque::new(),
            closing_existing: BTreeSet::new(),
            next_fiber_id: self.next_fiber_id,
            run_child_next: self.run_child_next,
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
        };
        if request_cancellation {
            for child in &self.child_fibers {
                let FlowFiberOwner::LineTask(owner) = &child.owner else {
                    continue;
                };
                if owner.tag.activation_id() != &activation_id {
                    continue;
                }
                match owner.cancel_policy {
                    ChildCancelPolicy::Finish => {}
                    ChildCancelPolicy::CancelAndJoin => {
                        batch.closing_existing.insert(child.id);
                    }
                    ChildCancelPolicy::Detach => unreachable!("cancel policy was preflighted"),
                }
            }
        }
        transaction
            .line_mut()
            .commit_scheduled_completion_stage(scheduled);
        for command in activation.commands {
            match command {
                crate::line_task::LineTaskCommand::Run { tag, policy } => {
                    let ops = group.command_ops(&tag);
                    let mut pending_ops = VecDeque::with_capacity(ops.len().saturating_add(2));
                    pending_ops.push_front(FlowOp::ExitScope);
                    for op in ops.iter().rev().cloned() {
                        pending_ops.push_front(op);
                    }
                    pending_ops.push_front(FlowOp::EnterScope {
                        identity: crate::scope::RuntimeScopeIdentity::Anonymous,
                    });
                    let selected_captures = if let Some(token) = tag.scheduled_token().cloned() {
                        let proof = packets
                            .remove(&token)
                            .expect("scheduled packet was preflighted once");
                        transaction
                            .line_mut()
                            .take_scheduled_capture_packet_prepared(proof)
                            .into_vec()
                    } else {
                        captures.to_vec()
                    };
                    let mut env = RuntimeEnv::default();
                    env.bind_all(selected_captures);
                    let ordinal = batch.next_fiber_id;
                    let allocated = ordinal
                        .checked_add(1)
                        .and_then(std::num::NonZeroU64::new)
                        .expect("fiber identity headroom was preflighted");
                    batch.next_fiber_id = batch
                        .next_fiber_id
                        .checked_add(1)
                        .expect("fiber identity headroom was preflighted");
                    batch.child_fibers.push_back(FlowFiber {
                        line_cursor: 0,
                        cursor: None,
                        pending_ops,
                        control_stack: Vec::new(),
                        await_observer: None,
                        root_cleanups: Vec::new(),
                        env,
                        observations: RuntimeObservationState::default(),
                        stream_states: BTreeMap::new(),
                        selected_dialogue_result: None,
                        id: FlowFiberId(ordinal),
                        persistent_id: RuntimePersistentFiberId::from_allocated(allocated.get()),
                        execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                            allocated,
                        ),
                        owner: FlowFiberOwner::LineTask(LineTaskFiberOwner {
                            tag,
                            join_policy: policy.join,
                            cancel_policy: policy.cancel,
                            closing: false,
                        }),
                        status: FlowFiberStatus::Running,
                    });
                    batch.run_child_next = true;
                }
                crate::line_task::LineTaskCommand::Cancel { tag } => {
                    for child in &self.child_fibers {
                        let FlowFiberOwner::LineTask(owner) = &child.owner else {
                            continue;
                        };
                        if owner.tag == tag {
                            match owner.cancel_policy {
                                ChildCancelPolicy::Finish => {}
                                ChildCancelPolicy::CancelAndJoin => {
                                    batch.closing_existing.insert(child.id);
                                }
                                ChildCancelPolicy::Detach => {
                                    unreachable!("existing child cancellation was preflighted")
                                }
                            }
                        }
                    }
                    for child in &mut batch.child_fibers {
                        let FlowFiberOwner::LineTask(owner) = &mut child.owner else {
                            continue;
                        };
                        if owner.tag == tag {
                            match owner.cancel_policy {
                                ChildCancelPolicy::Finish => {}
                                ChildCancelPolicy::CancelAndJoin => owner.closing = true,
                                ChildCancelPolicy::Detach => {
                                    unreachable!("planned child cancellation was preflighted")
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(batch.next_fiber_id, next_fiber_id);
        Ok(batch)
    }

    pub(super) fn commit_line_task_execution_batch(&mut self, batch: NativeLineTaskExecutionBatch) {
        for child in &mut self.child_fibers {
            if batch.closing_existing.contains(&child.id)
                && let FlowFiberOwner::LineTask(owner) = &mut child.owner
            {
                owner.closing = true;
            }
        }
        self.child_fibers.extend(batch.child_fibers);
        self.next_fiber_id = batch.next_fiber_id;
        self.run_child_next = batch.run_child_next;
        self.dialogue_effect_callback_activations = batch.dialogue_effect_callback_activations;
    }

    /// Checks the complete child ABI and handle projection while the owning
    /// registration is still in its activation or lexical defer stack.
    fn inspect_deferred_line_child(
        &self,
        registration: &crate::line_task::RuntimeLineDeferredRegistration,
    ) -> Result<NativePreparedDeferredLineChild, dialogue::DialogueExecutionError> {
        let captures = registration.captures();
        let site_id = self.plan.defer_function_site(registration.site()).ok_or(
            RuntimeEvalError::UnknownDeferredSite {
                site: registration.site(),
            },
        )?;
        let site = self
            .plan
            .validate_function_site_inputs(site_id, captures, &[])
            .map_err(RuntimeEvalError::from)?;
        if !matches!(site.body(), RuntimeFunctionSiteBody::Executable(_)) {
            return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation.into());
        }
        let mut capture_tokens = BTreeSet::new();
        for capture in captures {
            for handle in capture
                .affine_line_handles()
                .map_err(|_| crate::line_task::LineRuntimeError::InvalidDeferredTransition)?
            {
                if !capture_tokens.insert(handle.token().clone()) {
                    return Err(
                        crate::line_task::LineRuntimeError::DuplicateHandleOccurrence.into(),
                    );
                }
            }
        }
        let mut positions = BTreeSet::new();
        for input in site.inputs() {
            let RuntimeFunctionInputSource::Capture { position } = input.source() else {
                return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation.into());
            };
            if !positions.insert(position) {
                return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation.into());
            }
            let value = captures
                .get(position as usize)
                .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
            if !crate::pattern::inspect_runtime_pattern_owned(
                &self.plan,
                input.pattern(),
                value,
                site.type_instantiation.as_deref(),
            )? {
                return Err(RuntimeEvalError::PatternMismatch(format!(
                    "function site {site_id} deferred capture {:?}",
                    input.source()
                ))
                .into());
            }
            for handle in value
                .affine_line_handles()
                .map_err(|_| crate::line_task::LineRuntimeError::InvalidDeferredTransition)?
            {
                let destinations = crate::pattern::runtime_pattern_handle_destinations(
                    input.pattern(),
                    value,
                    handle.token(),
                )
                .map_err(|_| crate::line_task::LineRuntimeError::InvalidDeferredTransition)?;
                if destinations.len() != 1 {
                    return Err(
                        crate::line_task::LineRuntimeError::InvalidDeferredTransition.into(),
                    );
                }
            }
        }
        if positions.len() != captures.len() {
            return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation.into());
        }
        let ordinal = self.next_fiber_id;
        let next_fiber_id = ordinal
            .checked_add(1)
            .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
        let allocated = std::num::NonZeroU64::new(next_fiber_id)
            .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
        Ok(NativePreparedDeferredLineChild {
            type_instantiation: site.type_instantiation.clone(),
            site_id,
            capture_tokens,
            ordinal,
            allocated,
            next_fiber_id,
        })
    }

    /// Moves the sole checked capture packet into one child. All fallible
    /// lookup, pattern, token, and identity checks ran before the caller
    /// removed that packet from its stack.
    fn commit_prepared_deferred_line_child(
        &self,
        activation: &DialogueActivationId,
        registration: crate::line_task::RuntimeLineDeferredRegistration,
        prepared: NativePreparedDeferredLineChild,
    ) -> NativeLineTaskExecutionBatch {
        let (id, _, _, captures) = registration.into_parts();
        let site = self
            .plan
            .function_sites()
            .get(prepared.site_id)
            .expect("checked deferred function site remains present");
        let RuntimeFunctionSiteBody::Executable(body) = site.body() else {
            unreachable!("checked deferred function body remains executable")
        };
        let mut env = RuntimeEnv::default();
        env.push_function_scope(
            prepared.site_id,
            site.inputs().len(),
            prepared.type_instantiation.clone(),
        );
        let mut captures = captures.into_iter().map(Some).collect::<Vec<_>>();
        for input in site.inputs() {
            let RuntimeFunctionInputSource::Capture { position } = input.source() else {
                unreachable!("checked deferred input remains a capture")
            };
            let value = captures
                .get_mut(position as usize)
                .and_then(Option::take)
                .expect("checked deferred capture position remains unique");
            let bindings = crate::pattern::match_runtime_pattern_owned(
                &self.plan,
                input.pattern(),
                value,
                prepared.type_instantiation.as_deref(),
            )
            .expect("checked deferred pattern projection remains valid")
            .expect("checked deferred pattern remains matched");
            env.bind_all(bindings);
        }
        let mut pending_ops = VecDeque::with_capacity(body.ops().len().saturating_add(2));
        pending_ops.push_back(FlowOp::EnterScope {
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
        });
        pending_ops.extend(body.ops().iter().cloned());
        pending_ops.push_back(FlowOp::ExitScope);
        let mut batch = NativeLineTaskExecutionBatch {
            child_fibers: VecDeque::new(),
            closing_existing: BTreeSet::new(),
            next_fiber_id: prepared.next_fiber_id,
            run_child_next: true,
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
        };
        let child = FlowFiber {
            line_cursor: 0,
            cursor: None,
            pending_ops,
            control_stack: Vec::new(),
            await_observer: None,
            root_cleanups: Vec::new(),
            env,
            observations: RuntimeObservationState::default(),
            stream_states: BTreeMap::new(),
            selected_dialogue_result: None,
            id: FlowFiberId(prepared.ordinal),
            persistent_id: RuntimePersistentFiberId::from_allocated(prepared.allocated.get()),
            execution: crate::runtime_id::ExecutionInstanceId::from_allocated(prepared.allocated),
            owner: FlowFiberOwner::LineTask(LineTaskFiberOwner {
                tag: LineTaskWorkTag::activation(
                    activation.clone(),
                    crate::line_task::LineTaskWork::Defer(id),
                ),
                join_policy: ChildJoinPolicy::Join,
                cancel_policy: ChildCancelPolicy::Finish,
                closing: false,
            }),
            status: FlowFiberStatus::Running,
        };
        debug_assert_eq!(
            flow_fiber_line_handle_tokens(&child)
                .expect("checked deferred capture ownership remains unique"),
            prepared.capture_tokens
        );
        batch.child_fibers.push_back(child);
        batch
    }

    fn step_next_child_fiber(
        &mut self,
        input: &mut RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let image = match self.inert_rollback_image() {
            Ok(image) => image,
            Err(error) => {
                self.fail_eval(error, output);
                return true;
            }
        };
        let mut candidate = std::mem::replace(
            self,
            Self::new_with_shared_plan(Arc::clone(&self.plan), self.generation),
        );
        let mut staged_output = RuntimeStepOutput::default();
        match candidate.step_next_child_fiber_candidate(
            input,
            events,
            &mut staged_output,
            pure_backend,
        ) {
            Ok(progressed) => {
                *self = candidate;
                output.merge(staged_output);
                progressed
            }
            Err(error) => {
                drop(candidate);
                *self = Self::from_rollback_image(image)
                    .expect("a native Engine rollback image reconstructs its admitted owner");
                self.fail_eval(error, output);
                true
            }
        }
    }

    fn step_next_child_fiber_candidate(
        &mut self,
        input: &mut RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<bool, dialogue::DialogueExecutionError> {
        let Some(mut child) = self.child_fibers.pop_front() else {
            return Ok(false);
        };
        let owner = child.owner.clone();
        let before_tokens = matches!(&owner, FlowFiberOwner::LineTask(_))
            .then(|| flow_fiber_line_handle_tokens(&child))
            .transpose()?;
        std::mem::swap(&mut self.fiber, &mut child);
        let mut drop_policy = None;
        self.step_active_child_fiber(input, events, output, pure_backend, &mut drop_policy);
        self.finish_active_child_if_exhausted();
        std::mem::swap(&mut self.fiber, &mut child);
        let live_tokens = matches!(&owner, FlowFiberOwner::LineTask(_))
            .then(|| flow_fiber_line_handle_tokens(&child))
            .transpose()?;
        let mut drops = child.env.take_assignment_discard_authorization();
        drops.set_boundary(drop_policy)?;
        if let FlowFiberOwner::LineTask(owner) = &owner {
            let mut transaction = self
                .dialogue_activations
                .begin_transaction(owner.tag.activation_id())?;
            transaction.line_mut().reconcile_child_scope_step(
                &owner.tag,
                before_tokens
                    .as_ref()
                    .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?,
                live_tokens
                    .as_ref()
                    .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?,
                &drops,
            )?;
            let receipt = self.dialogue_activations.commit_transaction(transaction)?;
            Self::publish_dialogue_line_receipt(receipt.into_line(), output);
        }
        match child.status {
            FlowFiberStatus::Done(_) => {
                if let FlowFiberOwner::LineTask(owner) = owner {
                    let live_tokens = live_tokens
                        .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
                    if matches!(owner.tag.work(), crate::line_task::LineTaskWork::Defer(_)) {
                        self.complete_deferred_line_child(&owner.tag, &live_tokens, output)?;
                    } else {
                        let returned_bindings = std::mem::take(&mut child.env)
                            .into_slots()
                            .into_boxed_slice();
                        self.complete_line_task_work(
                            &owner.tag,
                            returned_bindings,
                            &live_tokens,
                            child.selected_dialogue_result.take(),
                            false,
                            owner.closing,
                            owner.join_policy == ChildJoinPolicy::Join,
                            output,
                        )?;
                    }
                }
            }
            FlowFiberStatus::Failed(message) => {
                let failed_activation = match &owner {
                    FlowFiberOwner::LineTask(owner)
                        if owner.join_policy == ChildJoinPolicy::Join =>
                    {
                        Some(owner.tag.activation_id().clone())
                    }
                    FlowFiberOwner::Executor | FlowFiberOwner::LineTask(_) => None,
                };
                if let FlowFiberOwner::LineTask(owner) = owner {
                    let live_tokens = live_tokens
                        .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
                    if matches!(owner.tag.work(), crate::line_task::LineTaskWork::Defer(_)) {
                        self.complete_deferred_line_child(&owner.tag, &live_tokens, output)?;
                    } else {
                        let returned_bindings = std::mem::take(&mut child.env)
                            .into_slots()
                            .into_boxed_slice();
                        self.complete_line_task_work(
                            &owner.tag,
                            returned_bindings,
                            &live_tokens,
                            None,
                            true,
                            false,
                            owner.join_policy == ChildJoinPolicy::Join,
                            output,
                        )?;
                    }
                }
                if let Some(activation) = failed_activation {
                    let transaction = self.dialogue_activations.begin_transaction(&activation)?;
                    self.begin_dialogue_failure(
                        transaction,
                        dialogue::DialogueExecutionError::ChildFailed { message },
                        output,
                    );
                }
            }
            _ => self.child_fibers.push_back(child),
        }
        Ok(true)
    }

    fn complete_deferred_line_child(
        &mut self,
        tag: &LineTaskWorkTag,
        live_tokens: &BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        output: &mut RuntimeStepOutput,
    ) -> Result<(), dialogue::DialogueExecutionError> {
        let crate::line_task::LineTaskWork::Defer(id) = tag.work() else {
            return Err(crate::line_task::LineRuntimeError::InvalidDeferredTransition.into());
        };
        match &self.fiber.status {
            FlowFiberStatus::Dialogue(activation)
                if activation.activation() == tag.activation_id() => {}
            _ => return Err(crate::line_task::LineRuntimeError::StaleCommandOutcome.into()),
        }
        let mut transaction = self
            .dialogue_activations
            .begin_transaction(tag.activation_id())?;
        if transaction
            .frame()
            .scopes
            .last()
            .and_then(|scope| scope.inflight)
            .is_some_and(|(inflight, _)| inflight == id)
        {
            let (frame, line) = transaction.parts_mut();
            let scope = frame.scopes.last_mut().expect("checked scoped defer");
            line.complete_scoped_deferred_child(tag.activation_id(), id, live_tokens)?;
            scope.inflight = None;
            let receipt = self.dialogue_activations.commit_transaction(transaction)?;
            Self::publish_dialogue_line_receipt(receipt.into_line(), output);
            return Ok(());
        }
        let (_, site) = transaction
            .line()
            .deferred_inflight()
            .filter(|(inflight, _)| *inflight == id)
            .ok_or(crate::line_task::LineRuntimeError::InvalidDeferredTransition)?;
        transaction.line_mut().complete_deferred_child(
            tag.activation_id(),
            id,
            site,
            live_tokens,
        )?;
        let receipt = self.dialogue_activations.commit_transaction(transaction)?;
        Self::publish_dialogue_line_receipt(receipt.into_line(), output);
        Ok(())
    }

    fn complete_line_task_work(
        &mut self,
        tag: &LineTaskWorkTag,
        returned_bindings: Box<[crate::value::RuntimeLocalSlot]>,
        live_tokens: &BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        selected_result: Option<RuntimeValue>,
        failed: bool,
        cancelled: bool,
        joined: bool,
        output: &mut RuntimeStepOutput,
    ) -> Result<(), dialogue::DialogueExecutionError> {
        let content = self
            .plan
            .dialogue_content()
            .get(tag.activation_id().content())
            .ok_or(crate::line_task::LineRuntimeError::UnknownContentPlan)?;
        let group_id = content
            .line_task_group()
            .ok_or(crate::line_task::LineRuntimeError::MissingTaskGroup)?;
        let group = self
            .plan
            .line_task_groups()
            .get(group_id.index())
            .cloned()
            .ok_or(crate::line_task::LineRuntimeError::UnknownTaskGroup)?;
        match &self.fiber.status {
            FlowFiberStatus::Dialogue(activation)
                if activation.activation() == tag.activation_id() => {}
            _ => return Err(crate::line_task::LineRuntimeError::StaleCommandOutcome.into()),
        }
        let mut transaction = self
            .dialogue_activations
            .begin_transaction(tag.activation_id())?;
        let (captures, mut live) = {
            let frame = transaction.frame();
            let dialogue::DialogueLineTaskState::Live(live) = &frame.line_task else {
                return Err(crate::line_task::LineRuntimeError::InvalidScheduledWorkState.into());
            };
            if frame
                .task_inputs
                .iter()
                .any(|binding| !binding.value.ownership().permits_copy())
            {
                return Err(crate::line_task::LineRuntimeError::AffineGroupCapture.into());
            }
            (frame.task_inputs.clone(), live.clone())
        };
        let mut selected_tokens = BTreeSet::new();
        if let Some(value) = selected_result {
            if failed || !joined || !live.accepts_result_selection(&group, tag) {
                return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation.into());
            }
            for handle in value
                .affine_line_handles()
                .map_err(|_| crate::line_task::LineRuntimeError::InvalidHandlePayload)?
            {
                if !selected_tokens.insert(handle.token().clone()) {
                    return Err(
                        crate::line_task::LineRuntimeError::DuplicateHandleOccurrence.into(),
                    );
                }
            }
            if !selected_tokens.is_subset(live_tokens) {
                return Err(
                    crate::line_task::LineRuntimeError::UnexpectedChildHandleOccurrence.into(),
                );
            }
            let ty = transaction.frame().result_target.ty();
            let checked = self
                .plan
                .checked_type(ty)
                .map_err(|_| crate::line_task::LineRuntimeError::ResultPatternOrTypeMismatch)?
                .ok_or(crate::line_task::LineRuntimeError::ResultPatternOrTypeMismatch)?;
            if !checked.accepts_value(&value) {
                return Err(crate::line_task::LineRuntimeError::ResultPatternOrTypeMismatch.into());
            }
            transaction.line_mut().select_result(tag, ty, value)?;
        }
        let next = if joined {
            crate::line_task::complete_live_line_task_work(&group, &mut live, tag.clone(), failed)?
        } else {
            crate::line_task::LineTaskActivation::default()
        };
        if let Some(token) = tag.scheduled_token().cloned() {
            let terminal = if failed {
                crate::line_task::RuntimeScheduledState::Failed
            } else if cancelled {
                crate::line_task::RuntimeScheduledState::Cancelled
            } else {
                crate::line_task::RuntimeScheduledState::Completed
            };
            let mut returned_tokens = BTreeSet::new();
            let mut surviving_bindings = Vec::new();
            for binding in returned_bindings {
                let handles = binding
                    .storage()
                    .values()
                    .map(|value| {
                        value.affine_line_handles().map_err(|_| {
                            crate::line_task::LineRuntimeError::InvalidScheduledCaptureGraph
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                if handles
                    .iter()
                    .any(|handle| selected_tokens.contains(handle.token()))
                {
                    continue;
                }
                for handle in handles {
                    if !returned_tokens.insert(handle.token().clone()) {
                        return Err(
                            crate::line_task::LineRuntimeError::DuplicateHandleOccurrence.into(),
                        );
                    }
                }
                surviving_bindings.push(binding);
            }
            let remaining_live_tokens = live_tokens
                .difference(&selected_tokens)
                .cloned()
                .collect::<BTreeSet<_>>();
            transaction.line_mut().finish_child_scope(
                tag,
                &remaining_live_tokens,
                &returned_tokens,
                crate::effect::RuntimeDropPolicy::Default,
            )?;
            transaction.line_mut().admit_scheduled_child_bindings(
                &token,
                surviving_bindings.into_boxed_slice(),
                terminal,
            )?;
            transaction
                .line_mut()
                .complete_scheduled_work(&token, failed, cancelled)?;
        }
        if joined {
            transaction.frame_mut().line_task = dialogue::DialogueLineTaskState::Live(live);
        }
        let batch =
            self.prepare_line_task_commands(&mut transaction, &group, next, &captures, false)?;
        let receipt = self.dialogue_activations.commit_transaction(transaction)?;
        Self::publish_dialogue_line_receipt(receipt.into_line(), output);
        self.commit_line_task_execution_batch(batch);
        Ok(())
    }

    fn step_active_child_fiber(
        &mut self,
        input: &mut RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
        drop_policy: &mut Option<crate::effect::RuntimeDropPolicy>,
    ) {
        if self.fiber.owner.requests_line_task_close() {
            self.close_active_line_task_fiber(output, pure_backend);
            return;
        }
        self.latch_active_await_observer_publications();
        if self.resume_suspended(input, events, output, pure_backend) {
            return;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return;
        }
        if self.fiber.cursor.is_some() || !self.fiber.pending_ops.is_empty() {
            self.step_flow(output, pure_backend, drop_policy);
        }
    }

    fn close_active_line_task_fiber(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let frames = std::mem::take(&mut self.fiber.control_stack);
        for frame in frames.into_iter().rev() {
            if let FlowControlStackEntryKind::Scope { cleanups, .. } = frame.kind {
                self.emit_scope_cleanups(cleanups, output, pure_backend);
            }
        }
        self.drain_root_cleanups(output, pure_backend);
        self.fiber.cursor = None;
        self.fiber.pending_ops.clear();
        self.fiber.await_observer = None;
        self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
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
        output
            .diagnostics
            .push(RuntimeDiagnostic::new(error.to_string()));
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
        if self.has_executor_work()
            && matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_)
                    | FlowFiberStatus::NeedWaiting(_)
                    | FlowFiberStatus::WaitingMany(_)
                    | FlowFiberStatus::HostCall(_)
                    | FlowFiberStatus::Dialogue(_)
                    | FlowFiberStatus::Choice(_)
            )
        {
            FlowFiberStatus::Running
        } else {
            match &self.fiber.status {
                FlowFiberStatus::Running => FlowFiberStatus::Running,
                FlowFiberStatus::Dialogue(activation) => {
                    FlowFiberStatus::Dialogue(activation.clone())
                }
                FlowFiberStatus::NeedWaiting(state) => {
                    FlowFiberStatus::NeedWaiting(Box::new(AwaitState {
                        binding: state.binding.clone(),
                        handle: state.handle.clone(),
                        item_type: state.item_type,
                        observers: state.observers.clone(),
                        resume: state.resume,
                        observed_through: state.observed_through,
                        queued: VecDeque::new(),
                    }))
                }
                FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(state)) => {
                    FlowFiberStatus::WaitingMany(WaitingManyStatus::Observed(AwaitManyProgress {
                        task: state.base.correlation().task_id,
                        completed: state
                            .results
                            .iter()
                            .filter(|result| result.is_some())
                            .count(),
                        total: state.results.len(),
                    }))
                }
                FlowFiberStatus::WaitingMany(WaitingManyStatus::Observed(progress)) => {
                    FlowFiberStatus::WaitingMany(WaitingManyStatus::Observed(progress.clone()))
                }
                FlowFiberStatus::HostCall(state) => FlowFiberStatus::HostCall(state.clone()),
                FlowFiberStatus::Choice(state) => FlowFiberStatus::Choice(state.clone()),
                FlowFiberStatus::Done(exit) => FlowFiberStatus::Done(exit.clone()),
                FlowFiberStatus::Failed(message) => FlowFiberStatus::Failed(message.clone()),
            }
        }
    }

    fn hard_stop_reason(&self, output: &RuntimeStepOutput) -> Option<RuntimeStepStopReason> {
        if self.has_executor_work()
            && matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_)
                    | FlowFiberStatus::NeedWaiting(_)
                    | FlowFiberStatus::WaitingMany(_)
                    | FlowFiberStatus::HostCall(_)
                    | FlowFiberStatus::Dialogue(_)
                    | FlowFiberStatus::Choice(_)
            )
        {
            return None;
        }
        match self.fiber.status {
            FlowFiberStatus::Done(_) => Some(RuntimeStepStopReason::Done),
            FlowFiberStatus::Failed(_) => Some(RuntimeStepStopReason::Failed),
            FlowFiberStatus::HostCall(_) => Some(if has_host_requests(output) {
                RuntimeStepStopReason::Output
            } else {
                RuntimeStepStopReason::Blocked
            }),
            FlowFiberStatus::Dialogue(_)
            | FlowFiberStatus::WaitingMany(_)
            | FlowFiberStatus::Choice(_) => Some(if has_presentation_visible_output(output) {
                RuntimeStepStopReason::Output
            } else {
                RuntimeStepStopReason::Blocked
            }),
            FlowFiberStatus::NeedWaiting(_) => Some(RuntimeStepStopReason::Blocked),
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
        || !output.requests.audio.is_empty()
        || !output.requests.cancel_scopes.is_empty()
        || !output.requests.ensure_content.is_empty()
        || !output.requests.host_calls.is_empty()
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

#[cfg(test)]
mod rollback_tests {
    use super::*;
    use crate::plan::RuntimePlanBuilder;
    use crate::task::{LogicalEpoch, RuntimeNeedState, TaskSequence};
    use arcweft_need::Need;
    use std::num::NonZeroU32;

    #[test]
    fn native_static_rollback_rejects_nested_affine_literal_before_copy() {
        let owner = crate::task::RuntimeProgramOwner::Plan(Arc::new(
            RuntimePlanBuilder::new().finish().unwrap(),
        ));
        let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
        let make_op = |value| FlowOp::If {
            condition: RuntimeExpr::from_admitted_parts(
                ty,
                crate::value::RuntimeExprKind::Value(RuntimeValue::Bool(true)),
            ),
            then_ops: vec![FlowOp::ReturnExpr(RuntimeExpr::from_admitted_parts(
                ty,
                crate::value::RuntimeExprKind::Value(value),
            ))],
            else_ops: vec![],
        };
        let affine = make_op(RuntimeValue::NeedHandle(crate::tests::reusable_need(
            "need.rollback",
        )));
        assert_eq!(
            FlowOpRollbackImage::from_live(&affine, &owner).unwrap_err(),
            "static flow operation contains an affine literal"
        );
        let unrestricted = make_op(RuntimeValue::Bool(false));
        let image = FlowOpRollbackImage::from_live(&unrestricted, &owner).unwrap();
        assert_eq!(image.into_live(&owner).unwrap(), unrestricted);
    }

    #[test]
    fn native_rollback_round_trips_distinct_affine_env_and_ready_owners() {
        use crate::plan::{
            RuntimeLocalDeclarationSeed, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
        };
        let unit = crate::pattern::RuntimeSemanticTypeId::from_bytes([0xe1; 32]);
        let need = crate::pattern::RuntimeSemanticTypeId::from_bytes([0xe2; 32]);
        let mut builder = RuntimePlanBuilder::new();
        let admitted = builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                    RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(unit)),
                ],
                [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-core.fixture.engine.native_rollback_round_trips_distinct_affine_env_and_ready_owners.binding_a"), need)],
            )
            .expect("admitted affine local");
        let pattern = builder
            .lower_pattern_seed_for_test(crate::plan::RuntimePatternSeed::new(
                need,
                crate::plan::RuntimePatternSeedKind::Bind {
                    mutable: false,
                    local: admitted.local_ids()[0].clone(),
                },
            ))
            .unwrap();
        let local = pattern.binding_declarations().next().unwrap().local();
        builder
            .declare_test_input_locals(admitted.local_ids())
            .unwrap();
        let plan = builder.finish().expect("typed owner plan");
        let mut engine = Engine::new(plan);
        let local_need = crate::tests::reusable_need_with_outcome(
            "local input",
            crate::task::TaskOutcomeContract::program(unit),
        );
        engine.fiber.env.bind_all_root([RuntimeLocalBinding {
            local,
            value: RuntimeValue::NeedHandle(local_need.clone()),
        }]);
        let mut ready_spec = crate::tests::task_spec(
            crate::task::TaskOutcomeContract::program(need),
            crate::task::HostTaskRequest::custom("fixture", "ready", []),
        );
        ready_spec.policy = TaskPolicy::AlwaysStart;
        ready_spec.generation = engine.generation;
        let ready_correlation = engine
            .need_producers
            .ensure_task(ready_spec)
            .unwrap()
            .handle()
            .correlation;
        let ready_value = RuntimeValue::NeedHandle(crate::tests::reusable_need_with_outcome(
            "ready payload",
            crate::task::TaskOutcomeContract::program(unit),
        ));
        assert!(!ready_value.ownership().permits_copy());
        let published = RuntimeNeedState::new(
            ready_correlation,
            Some(TaskPublicationCursor {
                logical_epoch: LogicalEpoch(1),
                sequence: TaskSequence(1),
            }),
            Need::Ready(crate::task::RuntimeNeedOutcome::Value(RuntimePayload(
                ready_value,
            ))),
        );
        let cursor = published.cursor.unwrap();
        let (correlation, _, state) = published.into_parts();
        assert!(engine.enqueue_need_publication(
            RuntimeNeedPublication::State {
                correlation,
                state,
                cursor,
            },
            &mut RuntimeStepOutput::default(),
        ));

        let image = engine.inert_rollback_image().expect("complete inert image");
        drop(engine);
        let restored = Engine::from_rollback_image(image).expect("exact owner restore");
        assert_eq!(
            restored.fiber.env.get(local),
            Some(&RuntimeValue::NeedHandle(local_need))
        );
        let queued = restored.need_publications.get(&ready_correlation).unwrap();
        assert_eq!(queued.len(), 1);
        assert!(matches!(
            &queued[0],
            RuntimeNeedPublication::State {
                state: Need::Ready(crate::task::RuntimeNeedOutcome::Value(value)),
                ..
            } if value.value() == &RuntimeValue::NeedHandle(crate::tests::reusable_need_with_outcome("ready payload",
                crate::task::TaskOutcomeContract::program(unit)))
        ));
        assert_eq!(restored.latest_need_publications.len(), 1);
    }

    #[test]
    fn native_observation_rollback_reconstructs_the_exact_plan_callable_owner() {
        let plan = crate::tests::function_application::returning_function_plan(
            crate::plan::RuntimeFunctionSiteBodyKind::Expression,
        );
        let mut engine = Engine::new(plan);
        let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
        let state = crate::tests::function_application::returning_callable_state(&engine.plan);
        let callable = crate::value::RuntimeCallableValue::try_new(owner.clone(), state, [])
            .expect("the original Plan admits its callable");
        engine
            .fiber
            .observations
            .record_effect(&LineEffectRequest::SignalWrite(
                crate::effect::RuntimeAssignment::try_new(
                    "signal.callback".to_owned(),
                    RuntimeValue::Tuple(vec![RuntimeValue::Callable(callable)]),
                )
                .expect("copyable callable is an unrestricted observation"),
            ));
        engine
            .fiber
            .observations
            .record_effect(&LineEffectRequest::MetricWrite(
                crate::effect::RuntimeAssignment::try_new(
                    "metric.count".to_owned(),
                    RuntimeValue::u64(u64::MAX),
                )
                .expect("unsigned metric remains typed"),
            ));
        let expected = crate::observation::RuntimeObservationSaveSnapshot::from_live_for_program(
            &engine.fiber.observations,
            &owner,
        )
        .expect("exact-owner observation image");
        let image = engine
            .inert_rollback_image()
            .expect("complete native rollback image");
        drop(engine);
        let restored =
            Engine::from_rollback_image(image).expect("the original native owner restores");
        let RuntimeValue::Tuple(values) =
            restored.fiber.observations.signals()["signal.callback"].value()
        else {
            panic!("the callable retains its enclosing typed tuple")
        };
        let RuntimeValue::Callable(callable) = &values[0] else {
            panic!("the tuple retains its callable")
        };
        assert!(callable.owner().same_program(&owner));
        assert_eq!(
            crate::observation::RuntimeObservationSaveSnapshot::from_live_for_program(
                &restored.fiber.observations,
                &owner,
            )
            .expect("restored image is admitted by the same owner"),
            expected,
        );

        let foreign_owner = crate::task::RuntimeProgramOwner::Plan(Arc::new(
            crate::tests::function_application::returning_function_plan(
                crate::plan::RuntimeFunctionSiteBodyKind::Expression,
            ),
        ));
        let error = crate::observation::RuntimeObservationSaveSnapshot::from_live_for_program(
            &restored.fiber.observations,
            &foreign_owner,
        )
        .expect_err("an equal-looking Plan cannot capture another native lease");
        assert_eq!(
            error,
            "observation 'signal.callback' cannot be saved: callable rollback image belongs to another program",
        );
    }
}

#[cfg(test)]
fn manual_local_source(declaration: &str) -> crate::plan::RuntimeLocalDeclarationSource {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    crate::plan::RuntimeLocalDeclarationSource::Binding {
        identity: *identity.finalize().as_bytes(),
        declaration: crate::plan::RuntimeLocalBindingDeclaration::new(
            crate::plan::RuntimeLocalBindingKind::PatternBinding,
            false,
            crate::plan::RuntimeLocalBindingStorage::Derived,
        ),
    }
}
