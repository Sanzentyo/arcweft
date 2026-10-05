//! Executor-neutral AWBC fiber and safe-point state.

use super::schema::{
    AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcDialogueResultTarget, AwbcEffectPlanId,
    AwbcEntryId, AwbcEntryTarget, AwbcFrameLayoutId, AwbcFrameSlotRole, AwbcFunctionId,
    AwbcFunctionKind, AwbcHostCallId, AwbcInstruction, AwbcPatternId, AwbcProgram, AwbcRegisterId,
    AwbcResumePointId, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcScopeId, AwbcSignatureId,
    AwbcSourceMapId, AwbcStreamPlanId, AwbcTaskPlanId, AwbcTraitReceiverMode, AwbcTrapCode,
    AwbcTypeId, AwbcVariantIdentity,
};
use crate::entry::{FlowParameterCoordinate, RuntimeNominalTypeId};
use crate::pattern::RuntimeSemanticTypeId;
use crate::pattern::RuntimeVariantIdentity;
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableInputSource, RuntimeCallableTransition,
    RuntimeDialogueValueBinding,
};
use crate::runtime_id::{
    RuntimeFiberInstanceId, RuntimeFrameInstanceId, RuntimeIdCursor, RuntimeIdNamespace,
};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    AwbcRuntimeValueSnapshot, RuntimeArcErrorContextKind, RuntimeArcErrorContextPending,
    RuntimeBinding, RuntimeCallablePendingGroup, RuntimeCallablePendingGroupParts,
    RuntimeCallableValue, RuntimeCallableZeroArgInvocationProof, RuntimeFlowParameterBinding,
    RuntimeFormatContext, RuntimeIterator, RuntimePlaceStorage, RuntimeSeq, RuntimeValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

mod format;
pub use format::{AwbcFiberFormatAttemptStateSnapshot, FiberFormatAttemptState};

type AwbcSaveResult<T> = Result<T, crate::value::AwbcRuntimeValueSnapshotError>;

/// Authenticated origin of a fiber, independent of its current callee cursor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum AwbcFiberRoot {
    Entry(AwbcEntryId),
    Program(arcweft_id::runtime_program::RuntimePureProgramId),
    Function(AwbcFunctionId),
    Empty,
}

impl AwbcFiberRoot {
    pub const fn entry(self) -> Option<AwbcEntryId> {
        match self {
            Self::Entry(entry) => Some(entry),
            Self::Program(_) | Self::Function(_) | Self::Empty => None,
        }
    }

    fn validate(self, program: &AwbcProgram) -> Result<Option<AwbcFunctionId>, FiberStateError> {
        match self {
            Self::Entry(entry) => program
                .entries
                .get(entry.index())
                .map(|_| None)
                .ok_or(FiberStateError::UnknownEntry(entry.0)),
            Self::Program(id) => program
                .pure_program_binding(id)
                .map(|binding| Some(binding.function))
                .ok_or(FiberStateError::UnknownProgram(id)),
            Self::Function(function) => program
                .functions
                .get(function.index())
                .map(|_| Some(function))
                .ok_or(FiberStateError::UnknownFunction(function.0)),
            Self::Empty if program.entries.is_empty() => Ok(None),
            Self::Empty => Err(FiberStateError::InvalidFrame),
        }
    }
}

/// Complete state that may cross compact-VM and compiled-region boundaries.
#[derive(Debug, PartialEq)]
pub struct FiberState {
    pub instance: RuntimeFiberInstanceId,
    pub next_frame_instance: RuntimeIdCursor,
    /// Next per-fiber AwaitMany occurrence ordinal, persisted across safe points.
    pub generation: u64,
    pub root: AwbcFiberRoot,
    pub cursor: FiberCursor,
    pub frames: Vec<FiberFrame>,
    pub status: FiberStatus,
    pub suspension: Option<FiberSuspension>,
    pub terminal: Option<FiberTerminalValue>,
    /// Non-owning label retained after a return value is delivered to the caller.
    pub return_summary: Option<String>,
    pub budget: FiberBudget,
    pub line_cursor: u64,
    pub streams: Vec<FiberStreamState>,
}

/// Proof produced while a callback remains in its owning dialogue map. It is
/// consumed together with that callback only after the full effect batch has
/// passed preflight.
pub(crate) struct PreparedCallableCallbackActivation {
    function: AwbcFunctionId,
    input_layout: PreparedFunctionInputBinding,
    proof: RuntimeCallableZeroArgInvocationProof,
}

/// Function-frame positional slots sealed by borrowed input validation.
#[derive(Debug)]
pub(crate) struct PreparedFunctionInputBinding {
    function: AwbcFunctionId,
    type_instantiation: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
    parameter_registers: Box<[AwbcRegisterId]>,
}

impl PreparedFunctionInputBinding {
    pub(crate) const fn function(&self) -> AwbcFunctionId {
        self.function
    }
}

pub(crate) struct PreparedFiberResume {
    fiber: RuntimeFiberInstanceId,
    frame: RuntimeFrameInstanceId,
    current_cursor: FiberCursor,
    resume: AwbcResumePointId,
    target: FiberCursor,
}

pub(crate) struct PreparedYieldedInstruction {
    fiber: RuntimeFiberInstanceId,
    frame: RuntimeFrameInstanceId,
    cursor: FiberCursor,
    next_offset: u32,
}

pub(crate) struct PreparedYieldedRegisterWrite {
    instruction: PreparedYieldedInstruction,
    register: AwbcRegisterId,
}

/// Exact owner-return proof for values temporarily held by a yielded VM
/// observation. The proof binds the unadvanced instruction and its consumed
/// register coordinates so an error path can put the sole owners back before
/// failure cleanup reconciles the fiber.
pub(crate) struct PreparedYieldedOperandRestore {
    fiber: RuntimeFiberInstanceId,
    frame: RuntimeFrameInstanceId,
    cursor: FiberCursor,
    registers: Box<[AwbcRegisterId]>,
    types: Box<[AwbcTypeId]>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FiberCursor {
    pub function: AwbcFunctionId,
    pub block: AwbcBlockId,
    /// Offset of the next instruction, or the block length when its terminator is next.
    pub instruction_offset: u32,
}

#[derive(Debug, PartialEq)]
pub struct FiberFrame {
    pub instance: RuntimeFrameInstanceId,
    pub function: AwbcFunctionId,
    pub type_instantiation:
        Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    pub layout: AwbcFrameLayoutId,
    pub return_to: Option<FiberReturnPoint>,
    pub registers: Vec<RuntimePlaceStorage<RuntimeValue>>,
    /// Source-order formatter operands staged at the exact instruction site.
    pub format: Option<FiberFormatState>,
    /// Nested source-ordered Flow formatter transactions owned by this frame.
    pub format_attempts: Vec<FiberFormatAttemptState>,
    pub root_cleanups: Vec<FiberScopeCleanup>,
    pub root_defers: Vec<FiberDeferredRegistration>,
    pub scopes: Vec<FiberScope>,
}

/// Values already produced by a `FormatContent` instruction on this frame.
/// `None` before `next_operand` denotes a recoverable evaluation failure;
/// `None` at or after it denotes an operand not yet reached.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberFormatState {
    site: FiberCursor,
    format_context: RuntimeFormatContext,
    next_operand: usize,
    values: Vec<Option<RuntimeValue>>,
    first_recoverable: Option<String>,
}

impl FiberFormatState {
    pub const fn format_context(&self) -> &RuntimeFormatContext {
        &self.format_context
    }

    pub const fn site(&self) -> FiberCursor {
        self.site
    }

    pub const fn next_operand(&self) -> usize {
        self.next_operand
    }

    pub fn values(&self) -> &[Option<RuntimeValue>] {
        &self.values
    }

    pub fn first_recoverable(&self) -> Option<&str> {
        self.first_recoverable.as_deref()
    }
}

#[derive(Debug, PartialEq)]
pub struct FiberReturnPoint {
    /// Exact caller cursor to restore after the callee returns.
    ///
    /// Static calls resolve their declared resume point to this cursor before
    /// entering the callee. Dynamic calls may resume at the instruction after
    /// the call without requiring a synthetic block or resume-point record.
    pub cursor: FiberCursor,
    pub destination: Option<AwbcRegisterId>,
    pub continuation: FiberReturnContinuation,
}

/// Stable re-entry coordinate for a ProjectCall terminator.  The return
/// continuation stores this coordinate instead of cloning the whole call
/// payload; the verified program remains the sole payload authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcProjectCallSite {
    pub caller_function: AwbcFunctionId,
    pub block: AwbcBlockId,
}

/// Typed continuation owned by one function return boundary. Project-call
/// default and target stages are part of this union so staged values survive
/// executable suspension and save/restore without a parallel side table.
#[derive(Debug, PartialEq)]
pub enum FiberReturnContinuation {
    Ordinary,
    ProjectCallDefault {
        site: AwbcProjectCallSite,
        pending: RuntimeCallablePendingGroup,
    },
    ProjectCallTarget {
        site: AwbcProjectCallSite,
    },
    ApplyGroupDefault {
        pending: RuntimeCallablePendingGroup,
        destination: AwbcRegisterId,
    },
    /// The result of one formatter operand returns to its exact instruction.
    FormatOperand {
        site: FiberCursor,
        ordinal: usize,
    },
    /// The selected project DisplayText method returns to the exact
    /// FormatContent instruction after its already-staged operands.
    FormatDisplay {
        site: FiberCursor,
    },
    /// A verified instruction call returns to the following cursor. The
    /// program instruction at `site` owns the target and any receiver update.
    InstructionCall {
        site: FiberCursor,
    },
    ContextCallbackDefault {
        site: FiberCursor,
        pending: RuntimeArcErrorContextPending,
        callable_pending: RuntimeCallablePendingGroup,
    },
    ContextCallbackInvoke {
        site: FiberCursor,
        pending: RuntimeArcErrorContextPending,
        callable_state: crate::runtime_id::RuntimeCallableStateId,
    },
}

impl FiberReturnPoint {
    #[must_use]
    pub const fn ordinary(cursor: FiberCursor, destination: Option<AwbcRegisterId>) -> Self {
        Self {
            cursor,
            destination,
            continuation: FiberReturnContinuation::Ordinary,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberScope {
    pub id: AwbcScopeId,
    pub depth: u32,
    pub cleanups: Vec<FiberScopeCleanup>,
    pub defers: Vec<FiberDeferredRegistration>,
    /// Captures skipped by their fixed outcome filter while host releases are
    /// outstanding. Only typed token identities remain after the packet drops.
    pub defer_releasing: Vec<FiberDeferredRelease>,
    /// The lexical exit is fixed before the first deferred body starts.
    pub defer_exit: Option<crate::line_task::ScopeExit>,
    /// Exact defer child currently borrowing this scope's top registration.
    pub defer_inflight: Option<FiberDeferredInFlight>,
    /// First cleanup failure; remaining registrations still use `defer_exit`.
    pub defer_failure: Option<FiberTrap>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberDeferredInFlight {
    pub registration: crate::runtime_id::RuntimeDeferRegistrationId,
    pub site: crate::runtime_id::RuntimeDeferSiteId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberDeferredRelease {
    pub registration: crate::runtime_id::RuntimeDeferRegistrationId,
    pub site: crate::runtime_id::RuntimeDeferSiteId,
    pub tokens: Vec<crate::runtime_id::RuntimeLineHandleToken>,
}

/// Captured values retained at one reached executable defer statement.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberDeferredRegistration {
    pub id: crate::runtime_id::RuntimeDeferRegistrationId,
    pub site: crate::runtime_id::RuntimeDeferSiteId,
    pub outcome: crate::line_task::RuntimeDeferOutcomeFilter,
    pub capture_registers: Vec<AwbcRegisterId>,
    pub captures: Vec<RuntimeValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberScopeCleanup {
    pub key: String,
    pub effect: AwbcEffectPlanId,
    pub args: Vec<RuntimeValue>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FiberStatus {
    Running,
    Suspended,
    Returned,
    Cancelled,
    Trapped,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberSuspension {
    pub resume: FiberResumeTarget,
    pub reason: FiberSuspensionReason,
}

/// Where a suspended fiber continues after the host replenishes or resolves it.
///
/// Program-declared suspension terminators use a verified resume point. Budget
/// preemption between instructions retains the exact execution cursor instead;
/// forcing that cursor through an unrelated declared point can replay work.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FiberResumeTarget {
    Declared(AwbcResumePointId),
    Exact(FiberCursor),
}

impl FiberSuspension {
    pub const fn declared_resume(&self) -> Option<AwbcResumePointId> {
        match self.resume {
            FiberResumeTarget::Declared(resume) => Some(resume),
            FiberResumeTarget::Exact(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum FiberSuspensionReason {
    Dialogue {
        /// Before Product admission this carries the terminator target. The
        /// Product dialogue registry owns it after presentation, so a live
        /// suspended fiber then retains `None` and cannot duplicate it.
        target: Option<crate::value::RuntimeOpaqueValue>,
        target_type: AwbcTypeId,
        content: AwbcContentUnitId,
        values: Box<[RuntimeDialogueValueBinding]>,
        effects: Box<[FiberDialogueContentEffectBinding]>,
        line_task_captures: Box<[RuntimeValue]>,
        result: AwbcDialogueResultTarget,
    },
    Choice {
        choice: AwbcChoiceId,
        destination: AwbcRegisterId,
    },
    Await {
        target: FiberAwaitTarget,
        binding: Option<AwbcPatternId>,
        observer: Option<crate::awbc::schema::AwbcAwaitObserverResume>,
    },
    AwaitMany(FiberAwaitManyState),
    HostCall {
        call: AwbcHostCallId,
        args: Vec<RuntimeValue>,
        destination: Option<AwbcRegisterId>,
    },
    BudgetYield,
}

/// Owned captures transferred from a Dialogue terminator into its suspension.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberDialogueContentEffectBinding {
    pub site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    pub state: crate::runtime_id::RuntimeCallableStateId,
    pub captures: Box<[RuntimeValue]>,
}

/// Exact await-handle authority retained by a suspended fiber.
///
/// Task handles keep the existing explicit task lifecycle. Need handles carry
/// only their typed identity; their Ready/Err payload is supplied through the
/// in-memory `RuntimeNeedState` step boundary rather than a runtime-value or
/// bytecode compatibility surrogate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum FiberAwaitTarget {
    Need {
        need: crate::task::RuntimeNeedHandle,
        item_type: AwbcTypeId,
        handle: AwbcRegisterId,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberAwaitManyState {
    pub plan: AwbcTaskPlanId,
    pub binding: Option<AwbcPatternId>,
    pub base: Option<crate::task::RuntimeNeedHandle>,
    pub captured: Vec<RuntimeValue>,
    pub items: Vec<RuntimeValue>,
    pub next_index: u32,
    pub in_flight: Vec<FiberAwaitManyInFlight>,
    pub results: Vec<Option<RuntimeValue>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberAwaitManyInFlight {
    pub index: u32,
    pub handle: crate::task::RuntimeNeedHandle,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberBudget {
    pub remaining: u64,
    pub quantum: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberStreamState {
    pub plan: AwbcStreamPlanId,
    pub queue: Vec<RuntimeValue>,
    pub closed: bool,
    pub emitted_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum FiberTerminalValue {
    Returned(Option<RuntimeValue>),
    DialogueResultSelected(RuntimeValue),
    Cancelled,
    Trapped(FiberTrap),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FiberTrap {
    pub code: AwbcTrapCode,
    pub message: Option<String>,
    pub source_map: Option<AwbcSourceMapId>,
}

/// A portable snapshot accepted by a compiled region at one verified safe point.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberSafePoint {
    pub generation: u64,
    pub cursor: FiberCursor,
    pub frame_layout: AwbcFrameLayoutId,
    pub resume: Option<AwbcResumePointId>,
}

/// Inert rollback snapshot used to guarantee effect-free VM fallback.
///
/// Unlike `FiberState`, this value contains only typed persistence DTOs and
/// never owns live runtime values. Restoring it replaces the current fiber;
/// callers must not activate a second runnable copy from the same checkpoint.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberCheckpoint {
    state: Box<AwbcFiberStateSnapshot>,
}

/// AWBC session-save projection of [`FiberState`].
///
/// Every live runtime-value slot is replaced by the explicit AWBC value DTO;
/// the live fiber is reconstructed only after the enclosing product has been
/// correlated with its generation-pinned program.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberStateSnapshot {
    pub instance: RuntimeFiberInstanceId,
    pub next_frame_instance: RuntimeIdCursor,
    pub generation: u64,
    pub root: AwbcFiberRoot,
    pub cursor: FiberCursor,
    pub frames: Vec<AwbcFiberFrameSnapshot>,
    pub status: FiberStatus,
    pub suspension: Option<AwbcFiberSuspensionSnapshot>,
    pub terminal: Option<AwbcFiberTerminalSnapshot>,
    pub return_summary: Option<String>,
    pub budget: FiberBudget,
    pub line_cursor: u64,
    pub streams: Vec<AwbcFiberStreamSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberFrameSnapshot {
    pub instance: RuntimeFrameInstanceId,
    pub function: AwbcFunctionId,
    pub type_instantiation: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
    pub layout: AwbcFrameLayoutId,
    pub return_to: Option<AwbcFiberReturnPointSnapshot>,
    pub registers: Vec<RuntimePlaceStorage<AwbcRuntimeValueSnapshot>>,
    pub format: Option<AwbcFiberFormatStateSnapshot>,
    pub format_attempts: Vec<AwbcFiberFormatAttemptStateSnapshot>,
    pub root_cleanups: Vec<AwbcFiberScopeCleanupSnapshot>,
    pub root_defers: Vec<AwbcFiberDeferredRegistrationSnapshot>,
    pub scopes: Vec<AwbcFiberScopeSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberFormatStateSnapshot {
    pub site: FiberCursor,
    pub format_context: RuntimeFormatContext,
    pub next_operand: usize,
    pub values: Vec<Option<AwbcRuntimeValueSnapshot>>,
    pub first_recoverable: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberReturnPointSnapshot {
    pub cursor: FiberCursor,
    pub destination: Option<AwbcRegisterId>,
    pub continuation: AwbcFiberReturnContinuationSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcFiberReturnContinuationSnapshot {
    Ordinary,
    ProjectCallDefault {
        site: AwbcProjectCallSite,
        pending: AwbcFiberCallablePendingSnapshot,
    },
    ProjectCallTarget {
        site: AwbcProjectCallSite,
    },
    ApplyGroupDefault {
        pending: AwbcFiberCallablePendingSnapshot,
        destination: AwbcRegisterId,
    },
    FormatOperand {
        site: FiberCursor,
        ordinal: usize,
    },
    FormatDisplay {
        site: FiberCursor,
    },
    InstructionCall {
        site: FiberCursor,
    },
    ContextCallbackDefault {
        site: FiberCursor,
        pending: AwbcFiberContextPendingSnapshot,
        callable_pending: AwbcFiberCallablePendingSnapshot,
    },
    ContextCallbackInvoke {
        site: FiberCursor,
        pending: AwbcFiberContextPendingSnapshot,
        callable_state: crate::runtime_id::RuntimeCallableStateId,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberCallablePendingSnapshot {
    pub state: crate::runtime_id::RuntimeCallableStateId,
    pub retained: Vec<Option<AwbcRuntimeValueSnapshot>>,
    pub arguments: Vec<Option<AwbcRuntimeValueSnapshot>>,
    pub attached: Option<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcFiberContextPendingSnapshot {
    ResultErr(AwbcRuntimeValueSnapshot),
    OptionNone,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberScopeSnapshot {
    pub id: AwbcScopeId,
    pub depth: u32,
    pub cleanups: Vec<AwbcFiberScopeCleanupSnapshot>,
    pub defers: Vec<AwbcFiberDeferredRegistrationSnapshot>,
    pub defer_releasing: Vec<FiberDeferredRelease>,
    pub defer_exit: Option<crate::line_task::ScopeExit>,
    pub defer_inflight: Option<FiberDeferredInFlight>,
    pub defer_failure: Option<FiberTrap>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberDeferredRegistrationSnapshot {
    pub id: crate::runtime_id::RuntimeDeferRegistrationId,
    pub site: crate::runtime_id::RuntimeDeferSiteId,
    pub outcome: crate::line_task::RuntimeDeferOutcomeFilter,
    pub capture_registers: Vec<AwbcRegisterId>,
    pub captures: Vec<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberScopeCleanupSnapshot {
    pub key: String,
    pub effect: AwbcEffectPlanId,
    pub args: Vec<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberSuspensionSnapshot {
    pub resume: FiberResumeTarget,
    pub reason: AwbcFiberSuspensionReasonSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcFiberSuspensionReasonSnapshot {
    Dialogue {
        target: Option<AwbcRuntimeValueSnapshot>,
        target_type: AwbcTypeId,
        content: AwbcContentUnitId,
        values: Box<[AwbcFiberDialogueValueBindingSnapshot]>,
        effects: Box<[AwbcFiberDialogueContentEffectBindingSnapshot]>,
        line_task_captures: Box<[AwbcRuntimeValueSnapshot]>,
        result: AwbcDialogueResultTarget,
    },
    Choice {
        choice: AwbcChoiceId,
        destination: AwbcRegisterId,
    },
    Await {
        target: AwbcFiberAwaitTargetSnapshot,
        binding: Option<AwbcPatternId>,
        observer: Option<crate::awbc::schema::AwbcAwaitObserverResume>,
    },
    AwaitMany(AwbcFiberAwaitManySnapshot),
    HostCall {
        call: AwbcHostCallId,
        args: Vec<AwbcRuntimeValueSnapshot>,
        destination: Option<AwbcRegisterId>,
    },
    BudgetYield,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberDialogueContentEffectBindingSnapshot {
    pub site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    pub state: crate::runtime_id::RuntimeCallableStateId,
    pub captures: Box<[AwbcRuntimeValueSnapshot]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberDialogueValueBindingSnapshot {
    pub slot: crate::runtime_id::RuntimeDialogueValueSlotId,
    pub role: crate::plan::RuntimeDialogueValueRole,
    pub value: AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcFiberAwaitTargetSnapshot {
    Need {
        need: crate::task::RuntimeNeedHandleSaveSnapshot,
        item_type: AwbcTypeId,
        handle: AwbcRegisterId,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberAwaitManySnapshot {
    pub plan: AwbcTaskPlanId,
    pub binding: Option<AwbcPatternId>,
    pub base: Option<crate::task::RuntimeNeedHandleSaveSnapshot>,
    pub captured: Vec<AwbcRuntimeValueSnapshot>,
    pub items: Vec<AwbcRuntimeValueSnapshot>,
    pub next_index: u32,
    pub in_flight: Vec<AwbcFiberAwaitManyChildSnapshot>,
    pub results: Vec<Option<AwbcRuntimeValueSnapshot>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberAwaitManyChildSnapshot {
    pub index: u32,
    pub handle: crate::task::RuntimeNeedHandleSaveSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberStreamSnapshot {
    pub plan: AwbcStreamPlanId,
    pub queue: Vec<AwbcRuntimeValueSnapshot>,
    pub closed: bool,
    pub emitted_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcFiberTerminalSnapshot {
    Returned(Option<AwbcRuntimeValueSnapshot>),
    DialogueResultSelected(AwbcRuntimeValueSnapshot),
    Cancelled,
    Trapped(FiberTrap),
}

impl AwbcFiberStateSnapshot {
    pub(crate) fn affine_line_handle_tokens(
        &self,
    ) -> Result<Vec<crate::runtime_id::RuntimeLineHandleToken>, String> {
        let mut tokens = Vec::new();
        for frame in &self.frames {
            for value in frame.registers.iter().flat_map(RuntimePlaceStorage::values) {
                extend_snapshot_line_handle_tokens(value, &mut tokens)?;
            }
            if let Some(format) = &frame.format {
                for value in format.values.iter().flatten() {
                    extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                }
            }
            for attempt in &frame.format_attempts {
                for value in attempt.values.iter().flatten() {
                    extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                }
            }
            if let Some(return_to) = &frame.return_to {
                match &return_to.continuation {
                    AwbcFiberReturnContinuationSnapshot::ProjectCallDefault { pending, .. }
                    | AwbcFiberReturnContinuationSnapshot::ApplyGroupDefault { pending, .. } => {
                        extend_callable_pending_line_handle_tokens(pending, &mut tokens)?;
                    }
                    AwbcFiberReturnContinuationSnapshot::ContextCallbackDefault {
                        pending,
                        callable_pending,
                        ..
                    } => {
                        extend_context_pending_line_handle_tokens(pending, &mut tokens)?;
                        extend_callable_pending_line_handle_tokens(callable_pending, &mut tokens)?;
                    }
                    AwbcFiberReturnContinuationSnapshot::ContextCallbackInvoke {
                        pending, ..
                    } => extend_context_pending_line_handle_tokens(pending, &mut tokens)?,
                    AwbcFiberReturnContinuationSnapshot::Ordinary
                    | AwbcFiberReturnContinuationSnapshot::ProjectCallTarget { .. }
                    | AwbcFiberReturnContinuationSnapshot::FormatOperand { .. }
                    | AwbcFiberReturnContinuationSnapshot::FormatDisplay { .. }
                    | AwbcFiberReturnContinuationSnapshot::InstructionCall { .. } => {}
                }
            }
            for cleanup in &frame.root_cleanups {
                for value in &cleanup.args {
                    extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                }
            }
            for registration in &frame.root_defers {
                for value in &registration.captures {
                    extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                }
            }
            for scope in &frame.scopes {
                for cleanup in &scope.cleanups {
                    for value in &cleanup.args {
                        extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                    }
                }
                for registration in &scope.defers {
                    for value in &registration.captures {
                        extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                    }
                }
            }
        }
        if let Some(suspension) = &self.suspension {
            match &suspension.reason {
                AwbcFiberSuspensionReasonSnapshot::Dialogue {
                    target,
                    values,
                    effects,
                    line_task_captures,
                    ..
                } => {
                    if let Some(target) = target {
                        extend_snapshot_line_handle_tokens(target, &mut tokens)?;
                    }
                    for binding in values.iter() {
                        extend_snapshot_line_handle_tokens(&binding.value, &mut tokens)?;
                    }
                    for effect in effects.iter() {
                        for value in effect.captures.iter() {
                            extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                        }
                    }
                    for value in line_task_captures.iter() {
                        extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                    }
                }
                AwbcFiberSuspensionReasonSnapshot::AwaitMany(state) => {
                    for value in &state.captured {
                        extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                    }
                    for handle in state
                        .base
                        .iter()
                        .chain(state.in_flight.iter().map(|child| &child.handle))
                    {
                        for value in handle.request_values() {
                            extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                        }
                    }
                    for value in &state.items {
                        extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                    }
                    for value in state.results.iter().flatten() {
                        extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                    }
                }
                AwbcFiberSuspensionReasonSnapshot::HostCall { args, .. } => {
                    for value in args {
                        extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                    }
                }
                AwbcFiberSuspensionReasonSnapshot::Choice { .. }
                | AwbcFiberSuspensionReasonSnapshot::Await { .. }
                | AwbcFiberSuspensionReasonSnapshot::BudgetYield => {}
            }
        }
        if let Some(terminal) = &self.terminal {
            match terminal {
                AwbcFiberTerminalSnapshot::Returned(value) => {
                    if let Some(value) = value.as_ref() {
                        extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                    }
                }
                AwbcFiberTerminalSnapshot::DialogueResultSelected(value) => {
                    extend_snapshot_line_handle_tokens(value, &mut tokens)?;
                }
                AwbcFiberTerminalSnapshot::Cancelled | AwbcFiberTerminalSnapshot::Trapped(_) => {}
            }
        }
        for stream in &self.streams {
            for value in &stream.queue {
                extend_snapshot_line_handle_tokens(value, &mut tokens)?;
            }
        }
        Ok(tokens)
    }

    pub fn from_live(state: &FiberState) -> AwbcSaveResult<Self> {
        Ok(Self {
            instance: state.instance,
            next_frame_instance: state.next_frame_instance,
            generation: state.generation,
            root: state.root,
            cursor: state.cursor,
            frames: state
                .frames
                .iter()
                .map(AwbcFiberFrameSnapshot::from_live)
                .collect::<Result<_, _>>()?,
            status: state.status,
            suspension: state
                .suspension
                .as_ref()
                .map(AwbcFiberSuspensionSnapshot::from_live)
                .transpose()?,
            terminal: state
                .terminal
                .as_ref()
                .map(AwbcFiberTerminalSnapshot::from_live)
                .transpose()?,
            return_summary: state.return_summary.clone(),
            budget: state.budget,
            line_cursor: state.line_cursor,
            streams: state
                .streams
                .iter()
                .map(AwbcFiberStreamSnapshot::from_live)
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn into_live_for_program(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberState> {
        Ok(FiberState {
            instance: self.instance,
            next_frame_instance: self.next_frame_instance,
            generation: self.generation,
            root: self.root,
            cursor: self.cursor,
            frames: self
                .frames
                .into_iter()
                .map(|value| value.into_live(owner))
                .collect::<Result<_, _>>()?,
            status: self.status,
            suspension: self
                .suspension
                .map(|value| value.into_live(owner))
                .transpose()?,
            terminal: self
                .terminal
                .map(|value| value.into_live(owner))
                .transpose()?,
            return_summary: self.return_summary,
            budget: self.budget,
            line_cursor: self.line_cursor,
            streams: self
                .streams
                .into_iter()
                .map(|value| value.into_live(owner))
                .collect::<Result<_, _>>()?,
        })
    }
}

fn extend_snapshot_line_handle_tokens(
    value: &AwbcRuntimeValueSnapshot,
    tokens: &mut Vec<crate::runtime_id::RuntimeLineHandleToken>,
) -> Result<(), String> {
    tokens.extend(
        value
            .affine_line_handle_tokens()
            .map_err(|error| error.to_string())?,
    );
    Ok(())
}

fn extend_callable_pending_line_handle_tokens(
    pending: &AwbcFiberCallablePendingSnapshot,
    tokens: &mut Vec<crate::runtime_id::RuntimeLineHandleToken>,
) -> Result<(), String> {
    for value in pending.retained.iter().chain(&pending.arguments).flatten() {
        extend_snapshot_line_handle_tokens(value, tokens)?;
    }
    if let Some(value) = &pending.attached {
        extend_snapshot_line_handle_tokens(value, tokens)?;
    }
    Ok(())
}

fn extend_context_pending_line_handle_tokens(
    pending: &AwbcFiberContextPendingSnapshot,
    tokens: &mut Vec<crate::runtime_id::RuntimeLineHandleToken>,
) -> Result<(), String> {
    if let AwbcFiberContextPendingSnapshot::ResultErr(value) = pending {
        extend_snapshot_line_handle_tokens(value, tokens)?;
    }
    Ok(())
}

impl AwbcFiberFrameSnapshot {
    fn from_live(frame: &FiberFrame) -> AwbcSaveResult<Self> {
        Ok(Self {
            instance: frame.instance,
            function: frame.function,
            type_instantiation: frame.type_instantiation.as_deref().cloned(),
            layout: frame.layout,
            return_to: frame
                .return_to
                .as_ref()
                .map(AwbcFiberReturnPointSnapshot::from_live)
                .transpose()?,
            registers: frame
                .registers
                .iter()
                .map(|value| value.try_map_ref(&mut AwbcRuntimeValueSnapshot::from_runtime_value))
                .collect::<Result<_, _>>()?,
            format: frame
                .format
                .as_ref()
                .map(AwbcFiberFormatStateSnapshot::from_live)
                .transpose()?,
            format_attempts: frame
                .format_attempts
                .iter()
                .map(AwbcFiberFormatAttemptStateSnapshot::from_live)
                .collect::<Result<_, _>>()?,
            root_cleanups: frame
                .root_cleanups
                .iter()
                .map(AwbcFiberScopeCleanupSnapshot::from_live)
                .collect::<Result<_, _>>()?,
            root_defers: frame
                .root_defers
                .iter()
                .map(AwbcFiberDeferredRegistrationSnapshot::from_live)
                .collect::<Result<_, _>>()?,
            scopes: frame
                .scopes
                .iter()
                .map(AwbcFiberScopeSnapshot::from_live)
                .collect::<Result<_, _>>()?,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberFrame> {
        Ok(FiberFrame {
            instance: self.instance,
            function: self.function,
            type_instantiation: self.type_instantiation.map(std::sync::Arc::new),
            layout: self.layout,
            return_to: self
                .return_to
                .map(|value| value.into_live(owner))
                .transpose()?,
            registers: self
                .registers
                .into_iter()
                .map(|value| {
                    let value =
                        value.try_map(&mut |value| value.into_runtime_value_for_program(owner))?;
                    value.validate_record_headers(owner).map_err(|message| {
                        crate::value::AwbcRuntimeValueSnapshotError::Message { message }
                    })?;
                    Ok(value)
                })
                .collect::<AwbcSaveResult<_>>()?,
            format: self
                .format
                .map(|value| value.into_live(owner))
                .transpose()?,
            format_attempts: self
                .format_attempts
                .into_iter()
                .map(|value| value.into_live(owner))
                .collect::<Result<_, _>>()?,
            root_cleanups: self
                .root_cleanups
                .into_iter()
                .map(|value| value.into_live(owner))
                .collect::<Result<_, _>>()?,
            root_defers: self
                .root_defers
                .into_iter()
                .map(|value| value.into_live(owner))
                .collect::<Result<_, _>>()?,
            scopes: self
                .scopes
                .into_iter()
                .map(|value| value.into_live(owner))
                .collect::<Result<_, _>>()?,
        })
    }
}

impl AwbcFiberFormatStateSnapshot {
    fn from_live(state: &FiberFormatState) -> AwbcSaveResult<Self> {
        Ok(Self {
            site: state.site,
            format_context: state.format_context.clone(),
            next_operand: state.next_operand,
            values: state
                .values
                .iter()
                .map(|value| {
                    value
                        .as_ref()
                        .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            first_recoverable: state.first_recoverable.clone(),
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberFormatState> {
        Ok(FiberFormatState {
            site: self.site,
            format_context: self.format_context,
            next_operand: self.next_operand,
            values: self
                .values
                .into_iter()
                .map(|value| {
                    value
                        .map(|value| value.into_runtime_value_for_program(owner))
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            first_recoverable: self.first_recoverable,
        })
    }
}

impl AwbcFiberReturnPointSnapshot {
    fn from_live(point: &FiberReturnPoint) -> AwbcSaveResult<Self> {
        Ok(Self {
            cursor: point.cursor,
            destination: point.destination,
            continuation: AwbcFiberReturnContinuationSnapshot::from_live(&point.continuation)?,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberReturnPoint> {
        Ok(FiberReturnPoint {
            cursor: self.cursor,
            destination: self.destination,
            continuation: self.continuation.into_live(owner)?,
        })
    }
}

impl AwbcFiberReturnContinuationSnapshot {
    fn from_live(continuation: &FiberReturnContinuation) -> AwbcSaveResult<Self> {
        Ok(match continuation {
            FiberReturnContinuation::Ordinary => Self::Ordinary,
            FiberReturnContinuation::ProjectCallDefault { site, pending } => {
                Self::ProjectCallDefault {
                    site: *site,
                    pending: AwbcFiberCallablePendingSnapshot::from_live(pending)?,
                }
            }
            FiberReturnContinuation::ProjectCallTarget { site } => {
                Self::ProjectCallTarget { site: *site }
            }
            FiberReturnContinuation::ApplyGroupDefault {
                pending,
                destination,
            } => Self::ApplyGroupDefault {
                pending: AwbcFiberCallablePendingSnapshot::from_live(pending)?,
                destination: *destination,
            },
            FiberReturnContinuation::FormatOperand { site, ordinal } => Self::FormatOperand {
                site: *site,
                ordinal: *ordinal,
            },
            FiberReturnContinuation::FormatDisplay { site } => Self::FormatDisplay { site: *site },
            FiberReturnContinuation::InstructionCall { site } => {
                Self::InstructionCall { site: *site }
            }
            FiberReturnContinuation::ContextCallbackDefault {
                site,
                pending,
                callable_pending,
            } => Self::ContextCallbackDefault {
                site: *site,
                pending: AwbcFiberContextPendingSnapshot::from_live(pending)?,
                callable_pending: AwbcFiberCallablePendingSnapshot::from_live(callable_pending)?,
            },
            FiberReturnContinuation::ContextCallbackInvoke {
                site,
                pending,
                callable_state,
            } => Self::ContextCallbackInvoke {
                site: *site,
                pending: AwbcFiberContextPendingSnapshot::from_live(pending)?,
                callable_state: *callable_state,
            },
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberReturnContinuation> {
        Ok(match self {
            Self::Ordinary => FiberReturnContinuation::Ordinary,
            Self::ProjectCallDefault { site, pending } => {
                FiberReturnContinuation::ProjectCallDefault {
                    site,
                    pending: pending.into_live(owner)?,
                }
            }
            Self::ProjectCallTarget { site } => FiberReturnContinuation::ProjectCallTarget { site },
            Self::ApplyGroupDefault {
                pending,
                destination,
            } => FiberReturnContinuation::ApplyGroupDefault {
                pending: pending.into_live(owner)?,
                destination,
            },
            Self::FormatOperand { site, ordinal } => {
                FiberReturnContinuation::FormatOperand { site, ordinal }
            }
            Self::FormatDisplay { site } => FiberReturnContinuation::FormatDisplay { site },
            Self::InstructionCall { site } => FiberReturnContinuation::InstructionCall { site },
            Self::ContextCallbackDefault {
                site,
                pending,
                callable_pending,
            } => FiberReturnContinuation::ContextCallbackDefault {
                site,
                pending: pending.into_live(owner)?,
                callable_pending: callable_pending.into_live(owner)?,
            },
            Self::ContextCallbackInvoke {
                site,
                pending,
                callable_state,
            } => FiberReturnContinuation::ContextCallbackInvoke {
                site,
                pending: pending.into_live(owner)?,
                callable_state,
            },
        })
    }
}

impl AwbcFiberContextPendingSnapshot {
    fn from_live(pending: &RuntimeArcErrorContextPending) -> AwbcSaveResult<Self> {
        Ok(match pending {
            RuntimeArcErrorContextPending::ResultErr(value) => {
                Self::ResultErr(AwbcRuntimeValueSnapshot::from_runtime_value(value)?)
            }
            RuntimeArcErrorContextPending::OptionNone => Self::OptionNone,
        })
    }

    fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<RuntimeArcErrorContextPending> {
        Ok(match self {
            Self::ResultErr(value) => RuntimeArcErrorContextPending::ResultErr(
                value.into_runtime_value_for_program(owner)?,
            ),
            Self::OptionNone => RuntimeArcErrorContextPending::OptionNone,
        })
    }
}

impl AwbcFiberCallablePendingSnapshot {
    fn from_live(pending: &RuntimeCallablePendingGroup) -> AwbcSaveResult<Self> {
        Ok(Self {
            state: pending.state(),
            retained: pending
                .retained()
                .iter()
                .map(|value| {
                    value
                        .as_ref()
                        .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            arguments: pending
                .arguments()
                .iter()
                .map(|value| {
                    value
                        .as_ref()
                        .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            attached: pending
                .attached()
                .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                .transpose()?,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<RuntimeCallablePendingGroup> {
        let parts = RuntimeCallablePendingGroupParts {
            owner: owner.clone(),
            state: self.state,
            retained: self
                .retained
                .into_iter()
                .map(|value| {
                    value
                        .map(|value| value.into_runtime_value_for_program(owner))
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            arguments: self
                .arguments
                .into_iter()
                .map(|value| {
                    value
                        .map(|value| value.into_runtime_value_for_program(owner))
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            attached: self
                .attached
                .map(|value| value.into_runtime_value_for_program(owner))
                .transpose()?,
        };
        RuntimeCallablePendingGroup::try_from_parts(parts).map_err(|error| {
            crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            }
        })
    }
}

impl AwbcFiberScopeSnapshot {
    fn from_live(scope: &FiberScope) -> AwbcSaveResult<Self> {
        Ok(Self {
            id: scope.id,
            depth: scope.depth,
            cleanups: scope
                .cleanups
                .iter()
                .map(AwbcFiberScopeCleanupSnapshot::from_live)
                .collect::<Result<_, _>>()?,
            defers: scope
                .defers
                .iter()
                .map(AwbcFiberDeferredRegistrationSnapshot::from_live)
                .collect::<Result<_, _>>()?,
            defer_releasing: scope.defer_releasing.clone(),
            defer_exit: scope.defer_exit,
            defer_inflight: scope.defer_inflight,
            defer_failure: scope.defer_failure.clone(),
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberScope> {
        Ok(FiberScope {
            id: self.id,
            depth: self.depth,
            cleanups: self
                .cleanups
                .into_iter()
                .map(|value| value.into_live(owner))
                .collect::<Result<_, _>>()?,
            defers: self
                .defers
                .into_iter()
                .map(|value| value.into_live(owner))
                .collect::<Result<_, _>>()?,
            defer_releasing: self.defer_releasing,
            defer_exit: self.defer_exit,
            defer_inflight: self.defer_inflight,
            defer_failure: self.defer_failure,
        })
    }
}

impl AwbcFiberScopeCleanupSnapshot {
    fn from_live(cleanup: &FiberScopeCleanup) -> AwbcSaveResult<Self> {
        Ok(Self {
            key: cleanup.key.clone(),
            effect: cleanup.effect,
            args: cleanup
                .args
                .iter()
                .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                .collect::<Result<_, _>>()?,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberScopeCleanup> {
        Ok(FiberScopeCleanup {
            key: self.key,
            effect: self.effect,
            args: self
                .args
                .into_iter()
                .map(|value| value.into_runtime_value_for_program(owner))
                .collect::<Result<_, _>>()?,
        })
    }
}

impl AwbcFiberDeferredRegistrationSnapshot {
    fn from_live(registration: &FiberDeferredRegistration) -> AwbcSaveResult<Self> {
        Ok(Self {
            id: registration.id,
            site: registration.site,
            outcome: registration.outcome,
            capture_registers: registration.capture_registers.clone(),
            captures: registration
                .captures
                .iter()
                .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                .collect::<Result<_, _>>()?,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberDeferredRegistration> {
        Ok(FiberDeferredRegistration {
            id: self.id,
            site: self.site,
            outcome: self.outcome,
            capture_registers: self.capture_registers,
            captures: self
                .captures
                .into_iter()
                .map(|value| value.into_runtime_value_for_program(owner))
                .collect::<Result<_, _>>()?,
        })
    }
}

impl AwbcFiberSuspensionSnapshot {
    fn from_live(suspension: &FiberSuspension) -> AwbcSaveResult<Self> {
        Ok(Self {
            resume: suspension.resume,
            reason: AwbcFiberSuspensionReasonSnapshot::from_live(&suspension.reason)?,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberSuspension> {
        Ok(FiberSuspension {
            resume: self.resume,
            reason: self.reason.into_live(owner)?,
        })
    }
}

impl AwbcFiberSuspensionReasonSnapshot {
    fn from_live(reason: &FiberSuspensionReason) -> AwbcSaveResult<Self> {
        Ok(match reason {
            FiberSuspensionReason::Dialogue {
                target,
                target_type,
                content,
                values,
                effects,
                line_task_captures,
                result,
            } => Self::Dialogue {
                target: target
                    .as_ref()
                    .map(AwbcRuntimeValueSnapshot::from_opaque)
                    .transpose()?,
                target_type: *target_type,
                content: *content,
                values: values
                    .iter()
                    .map(|binding| {
                        Ok(AwbcFiberDialogueValueBindingSnapshot {
                            slot: binding.slot,
                            role: binding.role,
                            value: AwbcRuntimeValueSnapshot::from_runtime_value(&binding.value)?,
                        })
                    })
                    .collect::<AwbcSaveResult<Vec<_>>>()?
                    .into_boxed_slice(),
                effects: effects
                    .iter()
                    .map(|effect| {
                        Ok(AwbcFiberDialogueContentEffectBindingSnapshot {
                            site: effect.site,
                            state: effect.state,
                            captures: effect
                                .captures
                                .iter()
                                .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                                .collect::<Result<Vec<_>, _>>()?
                                .into_boxed_slice(),
                        })
                    })
                    .collect::<AwbcSaveResult<Vec<_>>>()?
                    .into_boxed_slice(),
                line_task_captures: line_task_captures
                    .iter()
                    .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                result: result.clone(),
            },
            FiberSuspensionReason::Choice {
                choice,
                destination,
            } => Self::Choice {
                choice: *choice,
                destination: *destination,
            },
            FiberSuspensionReason::Await {
                target,
                binding,
                observer,
            } => Self::Await {
                target: AwbcFiberAwaitTargetSnapshot::from_live(target)?,
                binding: *binding,
                observer: *observer,
            },
            FiberSuspensionReason::AwaitMany(state) => {
                Self::AwaitMany(AwbcFiberAwaitManySnapshot::from_live(state)?)
            }
            FiberSuspensionReason::HostCall {
                call,
                args,
                destination,
            } => Self::HostCall {
                call: *call,
                args: args
                    .iter()
                    .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                    .collect::<Result<_, _>>()?,
                destination: *destination,
            },
            FiberSuspensionReason::BudgetYield => Self::BudgetYield,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberSuspensionReason> {
        Ok(match self {
            Self::Dialogue {
                target,
                target_type,
                content,
                values,
                effects,
                line_task_captures,
                result,
            } => {
                let target = target
                    .map(|target| {
                        let value = target.into_runtime_value_for_program(owner)?;
                        let RuntimeValue::Opaque(target) = value else {
                            return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                                message: "dialogue target snapshot is not an opaque value"
                                    .to_owned(),
                            });
                        };
                        Ok(target)
                    })
                    .transpose()?;
                FiberSuspensionReason::Dialogue {
                    target,
                    target_type,
                    content,
                    values: values
                        .into_vec()
                        .into_iter()
                        .map(|binding| {
                            Ok(RuntimeDialogueValueBinding {
                                slot: binding.slot,
                                role: binding.role,
                                value: binding.value.into_runtime_value_for_program(owner)?,
                            })
                        })
                        .collect::<AwbcSaveResult<Vec<_>>>()?
                        .into_boxed_slice(),
                    effects: effects
                        .into_iter()
                        .map(|effect| {
                            Ok(FiberDialogueContentEffectBinding {
                                site: effect.site,
                                state: effect.state,
                                captures: effect
                                    .captures
                                    .into_iter()
                                    .map(|value| value.into_runtime_value_for_program(owner))
                                    .collect::<Result<Vec<_>, _>>()?
                                    .into_boxed_slice(),
                            })
                        })
                        .collect::<AwbcSaveResult<Vec<_>>>()?
                        .into_boxed_slice(),
                    line_task_captures: line_task_captures
                        .into_iter()
                        .map(|value| value.into_runtime_value_for_program(owner))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                    result,
                }
            }
            Self::Choice {
                choice,
                destination,
            } => FiberSuspensionReason::Choice {
                choice,
                destination,
            },
            Self::Await {
                target,
                binding,
                observer,
            } => FiberSuspensionReason::Await {
                target: target.into_live(owner)?,
                binding,
                observer,
            },
            Self::AwaitMany(state) => FiberSuspensionReason::AwaitMany(state.into_live(owner)?),
            Self::HostCall {
                call,
                args,
                destination,
            } => FiberSuspensionReason::HostCall {
                call,
                args: args
                    .into_iter()
                    .map(|value| value.into_runtime_value_for_program(owner))
                    .collect::<Result<_, _>>()?,
                destination,
            },
            Self::BudgetYield => FiberSuspensionReason::BudgetYield,
        })
    }
}

impl AwbcFiberAwaitTargetSnapshot {
    fn from_live(target: &FiberAwaitTarget) -> AwbcSaveResult<Self> {
        Ok(match target {
            FiberAwaitTarget::Need {
                need,
                item_type,
                handle,
            } => Self::Need {
                need: crate::task::RuntimeNeedHandleSaveSnapshot::from_live(need, None)?,
                item_type: *item_type,
                handle: *handle,
            },
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberAwaitTarget> {
        Ok(match self {
            Self::Need {
                need,
                item_type,
                handle,
            } => FiberAwaitTarget::Need {
                need: need.into_live(owner)?,
                item_type,
                handle,
            },
        })
    }
}

impl AwbcFiberAwaitManySnapshot {
    fn from_live(state: &FiberAwaitManyState) -> AwbcSaveResult<Self> {
        Ok(Self {
            plan: state.plan,
            binding: state.binding,
            base: state
                .base
                .as_ref()
                .map(|handle| crate::task::RuntimeNeedHandleSaveSnapshot::from_live(handle, None))
                .transpose()?,
            captured: state
                .captured
                .iter()
                .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                .collect::<Result<_, _>>()?,
            items: state
                .items
                .iter()
                .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                .collect::<Result<_, _>>()?,
            next_index: state.next_index,
            in_flight: state
                .in_flight
                .iter()
                .map(|child| {
                    Ok(AwbcFiberAwaitManyChildSnapshot {
                        index: child.index,
                        handle: crate::task::RuntimeNeedHandleSaveSnapshot::from_live(
                            &child.handle,
                            None,
                        )?,
                    })
                })
                .collect::<AwbcSaveResult<_>>()?,
            results: state
                .results
                .iter()
                .map(|value| {
                    value
                        .as_ref()
                        .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberAwaitManyState> {
        Ok(FiberAwaitManyState {
            plan: self.plan,
            binding: self.binding,
            base: self
                .base
                .map(|handle| handle.into_live(owner))
                .transpose()?,
            captured: self
                .captured
                .into_iter()
                .map(|value| value.into_runtime_value_for_program(owner))
                .collect::<Result<_, _>>()?,
            items: self
                .items
                .into_iter()
                .map(|value| value.into_runtime_value_for_program(owner))
                .collect::<Result<_, _>>()?,
            next_index: self.next_index,
            in_flight: self
                .in_flight
                .into_iter()
                .map(|child| {
                    Ok(FiberAwaitManyInFlight {
                        index: child.index,
                        handle: child.handle.into_live(owner)?,
                    })
                })
                .collect::<AwbcSaveResult<_>>()?,
            results: self
                .results
                .into_iter()
                .map(|value| {
                    value
                        .map(|value| value.into_runtime_value_for_program(owner))
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

impl AwbcFiberStreamSnapshot {
    fn from_live(stream: &FiberStreamState) -> AwbcSaveResult<Self> {
        Ok(Self {
            plan: stream.plan,
            queue: stream
                .queue
                .iter()
                .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                .collect::<Result<_, _>>()?,
            closed: stream.closed,
            emitted_count: stream.emitted_count,
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberStreamState> {
        Ok(FiberStreamState {
            plan: self.plan,
            queue: self
                .queue
                .into_iter()
                .map(|value| value.into_runtime_value_for_program(owner))
                .collect::<Result<_, _>>()?,
            closed: self.closed,
            emitted_count: self.emitted_count,
        })
    }
}

impl AwbcFiberTerminalSnapshot {
    fn from_live(terminal: &FiberTerminalValue) -> AwbcSaveResult<Self> {
        Ok(match terminal {
            FiberTerminalValue::Returned(value) => Self::Returned(
                value
                    .as_ref()
                    .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                    .transpose()?,
            ),
            FiberTerminalValue::DialogueResultSelected(value) => {
                Self::DialogueResultSelected(AwbcRuntimeValueSnapshot::from_runtime_value(value)?)
            }
            FiberTerminalValue::Cancelled => Self::Cancelled,
            FiberTerminalValue::Trapped(trap) => Self::Trapped(trap.clone()),
        })
    }

    fn into_live(self, owner: &RuntimeProgramOwner) -> AwbcSaveResult<FiberTerminalValue> {
        Ok(match self {
            Self::Returned(value) => FiberTerminalValue::Returned(
                value
                    .map(|value| value.into_runtime_value_for_program(owner))
                    .transpose()?,
            ),
            Self::DialogueResultSelected(value) => FiberTerminalValue::DialogueResultSelected(
                value.into_runtime_value_for_program(owner)?,
            ),
            Self::Cancelled => FiberTerminalValue::Cancelled,
            Self::Trapped(trap) => FiberTerminalValue::Trapped(trap),
        })
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FiberStateError {
    #[error(transparent)]
    Snapshot(#[from] crate::value::AwbcRuntimeValueSnapshotError),
    #[error(transparent)]
    RuntimeIdentity(#[from] crate::runtime_id::RuntimeIdExhausted),
    #[error("AWBC entry {0} does not exist")]
    UnknownEntry(u32),
    #[error("AWBC program {0} does not exist")]
    UnknownProgram(arcweft_id::runtime_program::RuntimePureProgramId),
    #[error("AWBC entry target is a route set and needs a host-selected route")]
    RouteSelectionRequired,
    #[error("AWBC function {0} does not exist")]
    UnknownFunction(u32),
    #[error("AWBC frame layout {0} does not exist")]
    UnknownFrameLayout(u32),
    #[error("AWBC resume point {0} does not exist")]
    UnknownResumePoint(u32),
    #[error("AWBC resume point {resume} belongs to function {actual}, not {expected}")]
    ResumeFunctionMismatch {
        resume: u32,
        actual: u32,
        expected: u32,
    },
    #[error("AWBC resume point {resume} expects frame layout {actual}, not {expected}")]
    ResumeLayoutMismatch {
        resume: u32,
        actual: u32,
        expected: u32,
    },
    #[error("fiber has no active frame")]
    MissingFrame,
    #[error("fiber is {actual:?}; operation requires {expected:?}")]
    InvalidStatus {
        actual: FiberStatus,
        expected: FiberStatus,
    },
    #[error("fiber cursor is stale: observed {observed:?}, current {current:?}")]
    StaleCursor {
        observed: FiberCursor,
        current: FiberCursor,
    },
    #[error("fiber instruction offset overflowed at cursor {cursor:?}")]
    InstructionOffsetOverflow { cursor: FiberCursor },
    #[error("fiber register {register} does not exist in frame layout {layout}")]
    RegisterOutOfBounds { register: u32, layout: u32 },
    #[error("fiber register {register} is already initialized in frame layout {layout}")]
    RegisterAlreadyInitialized { register: u32, layout: u32 },
    #[error("fiber frame function/layout pair is invalid")]
    InvalidFrame,
    #[error("fiber call return value does not match its destination")]
    ReturnValueMismatch,
    #[error("invalid AWBC runtime callable: {reason}")]
    InvalidRuntimeCallable { reason: String },
    #[error("invalid runtime value at {path}: {reason}")]
    InvalidRuntimeValue { path: String, reason: String },
    #[error("{kind} has no admitted AWBC snapshot representation")]
    UnsupportedSnapshotValue { kind: &'static str },
    #[error("AWBC function expects {expected} arguments, received {actual}")]
    ArgumentCount { expected: usize, actual: usize },
    #[error("AWBC function argument `{name}` is duplicated")]
    DuplicateArgument { name: String },
    #[error("AWBC function argument `{name}` does not match a parameter")]
    UnknownArgument { name: String },
    #[error("AWBC function argument `{name}` expected {expected}, received {actual}")]
    ArgumentType {
        name: String,
        expected: String,
        actual: String,
    },
    #[error("AWBC function input ownership validation failed: {reason}")]
    InvalidFunctionInputOwnership { reason: String },
    #[error("AWBC function argument {position} is affine but this binding path copies it")]
    ArgumentNotCopyable { position: usize },
    #[error("AWBC Flow parameter coordinate {parameter:?} is out of range")]
    UnknownFlowParameter { parameter: FlowParameterCoordinate },
    #[error("AWBC Flow parameter coordinate {parameter:?} is duplicated")]
    DuplicateFlowParameter { parameter: FlowParameterCoordinate },
    #[error("AWBC Flow parameter {parameter:?} expected {expected}, received {actual}")]
    FlowParameterType {
        parameter: FlowParameterCoordinate,
        expected: String,
        actual: String,
    },
}

fn visit_value_graph<E>(
    value: &RuntimeValue,
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    crate::value::visit_runtime_value_graph(value, |nested| visitor(nested))
}

fn visit_value_slice<E>(
    values: &[RuntimeValue],
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    for value in values {
        visit_value_graph(value, visitor)?;
    }
    Ok(())
}

fn visit_callable_pending_values<E>(
    pending: &RuntimeCallablePendingGroup,
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    for value in pending
        .retained()
        .iter()
        .chain(pending.arguments())
        .filter_map(Option::as_ref)
    {
        visit_value_graph(value, visitor)?;
    }
    if let Some(value) = pending.attached() {
        visit_value_graph(value, visitor)?;
    }
    Ok(())
}

fn visit_scope_cleanup_values<E>(
    cleanups: &[FiberScopeCleanup],
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    for cleanup in cleanups {
        visit_value_slice(&cleanup.args, visitor)?;
    }
    Ok(())
}

fn visit_deferred_capture_values<E>(
    registrations: &[FiberDeferredRegistration],
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    for registration in registrations {
        visit_value_slice(&registration.captures, visitor)?;
    }
    Ok(())
}

impl FiberState {
    /// Completed root frames may retain Copy values and lexical metadata.
    /// Their affine owners and unfinished cleanup work cannot be discarded by
    /// a program continuation.
    pub(crate) fn program_continuation_ready(&self) -> bool {
        self.status == FiberStatus::Returned
            && matches!(self.root, AwbcFiberRoot::Program(_))
            && matches!(self.terminal, Some(FiberTerminalValue::Returned(Some(_))))
            && self.suspension.is_none()
            && self.frames.len() <= 1
            && self.frames.iter().all(|frame| {
                frame.return_to.is_none()
                    && frame.format.is_none()
                    && frame.format_attempts.is_empty()
                    && frame.root_cleanups.is_empty()
                    && frame.root_defers.is_empty()
                    && frame
                        .scopes
                        .iter()
                        .all(|scope| scope.cleanups.is_empty() && scope.defers.is_empty())
                    && frame
                        .registers
                        .iter()
                        .flat_map(|storage| storage.values())
                        .all(|value| value.ownership().permits_copy())
            })
    }
    /// Visits every runtime value retained by this fiber, including values in
    /// saved call continuations, defer captures, suspension payloads, streams,
    /// and terminal results.
    pub fn visit_runtime_values<E>(
        &self,
        mut visitor: impl FnMut(&RuntimeValue) -> Result<(), E>,
    ) -> Result<(), E> {
        for frame in &self.frames {
            for value in frame.registers.iter().flat_map(RuntimePlaceStorage::values) {
                visit_value_graph(value, &mut visitor)?;
            }
            if let Some(format) = &frame.format {
                for value in format.values.iter().flatten() {
                    visit_value_graph(value, &mut visitor)?;
                }
            }
            for attempt in &frame.format_attempts {
                for value in attempt.values().iter().flatten() {
                    visit_value_graph(value, &mut visitor)?;
                }
            }
            if let Some(return_to) = &frame.return_to {
                match &return_to.continuation {
                    FiberReturnContinuation::Ordinary
                    | FiberReturnContinuation::ProjectCallTarget { .. }
                    | FiberReturnContinuation::FormatOperand { .. }
                    | FiberReturnContinuation::FormatDisplay { .. }
                    | FiberReturnContinuation::InstructionCall { .. } => {}
                    FiberReturnContinuation::ProjectCallDefault { pending, .. }
                    | FiberReturnContinuation::ApplyGroupDefault { pending, .. } => {
                        visit_callable_pending_values(pending, &mut visitor)?;
                    }
                    FiberReturnContinuation::ContextCallbackDefault {
                        pending,
                        callable_pending,
                        ..
                    } => {
                        if let RuntimeArcErrorContextPending::ResultErr(cause) = pending {
                            visit_value_graph(cause, &mut visitor)?;
                        }
                        visit_callable_pending_values(callable_pending, &mut visitor)?;
                    }
                    FiberReturnContinuation::ContextCallbackInvoke { pending, .. } => {
                        if let RuntimeArcErrorContextPending::ResultErr(cause) = pending {
                            visit_value_graph(cause, &mut visitor)?;
                        }
                    }
                }
            }
            visit_scope_cleanup_values(&frame.root_cleanups, &mut visitor)?;
            visit_deferred_capture_values(&frame.root_defers, &mut visitor)?;
            for scope in &frame.scopes {
                visit_scope_cleanup_values(&scope.cleanups, &mut visitor)?;
                visit_deferred_capture_values(&scope.defers, &mut visitor)?;
            }
        }
        if let Some(suspension) = &self.suspension {
            match &suspension.reason {
                FiberSuspensionReason::Dialogue {
                    target,
                    values,
                    effects,
                    line_task_captures,
                    ..
                } => {
                    if let Some(target) = target {
                        visit_value_graph(target.payload(), &mut visitor)?;
                    }
                    for binding in values.iter() {
                        visit_value_graph(&binding.value, &mut visitor)?;
                    }
                    for effect in effects.iter() {
                        visit_value_slice(&effect.captures, &mut visitor)?;
                    }
                    visit_value_slice(line_task_captures, &mut visitor)?;
                }
                FiberSuspensionReason::AwaitMany(state) => {
                    visit_value_slice(&state.captured, &mut visitor)?;
                    for handle in state
                        .base
                        .iter()
                        .chain(state.in_flight.iter().map(|child| &child.handle))
                    {
                        for value in handle.request_values() {
                            visit_value_graph(value, &mut visitor)?;
                        }
                    }
                    visit_value_slice(&state.items, &mut visitor)?;
                    for value in state.results.iter().flatten() {
                        visit_value_graph(value, &mut visitor)?;
                    }
                }
                FiberSuspensionReason::HostCall { args, .. } => {
                    visit_value_slice(args, &mut visitor)?;
                }
                FiberSuspensionReason::Choice { .. }
                | FiberSuspensionReason::Await { .. }
                | FiberSuspensionReason::BudgetYield => {}
            }
        }
        for stream in &self.streams {
            visit_value_slice(&stream.queue, &mut visitor)?;
        }
        if let Some(terminal) = &self.terminal {
            match terminal {
                FiberTerminalValue::Returned(Some(value))
                | FiberTerminalValue::DialogueResultSelected(value) => {
                    visit_value_graph(value, &mut visitor)?;
                }
                FiberTerminalValue::Returned(None)
                | FiberTerminalValue::Cancelled
                | FiberTerminalValue::Trapped(_) => {}
            }
        }
        Ok(())
    }

    /// Visits completed values retained by inline or Flow-lowered formatter
    /// continuations with their exact dynamic frame and static instruction site.
    pub(crate) fn visit_formatter_operand_values<E>(
        &self,
        mut visitor: impl FnMut(
            RuntimeFrameInstanceId,
            FiberCursor,
            usize,
            &RuntimeValue,
        ) -> Result<(), E>,
    ) -> Result<(), E> {
        for frame in &self.frames {
            if let Some(format) = &frame.format {
                for (ordinal, value) in format.values.iter().enumerate() {
                    if let Some(value) = value.as_ref() {
                        visitor(frame.instance, format.site, ordinal, value)?;
                    }
                }
            }
            for attempt in &frame.format_attempts {
                for (ordinal, value) in attempt.values().iter().enumerate() {
                    if let Some(value) = value.as_ref() {
                        visitor(frame.instance, attempt.site(), ordinal, value)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Creates a root fiber for a function entrypoint.
    pub fn for_entry(
        program: &AwbcProgram,
        entry: AwbcEntryId,
        generation: u64,
        budget_quantum: u64,
    ) -> Result<Self, FiberStateError> {
        Self::for_entry_with_instance(
            program,
            entry,
            RuntimeFiberInstanceId::from_allocated(std::num::NonZeroU64::MIN),
            generation,
            budget_quantum,
        )
    }

    pub(crate) fn for_entry_with_instance(
        program: &AwbcProgram,
        entry: AwbcEntryId,
        instance: RuntimeFiberInstanceId,
        generation: u64,
        budget_quantum: u64,
    ) -> Result<Self, FiberStateError> {
        let entry_record = program
            .entries
            .get(entry.index())
            .ok_or(FiberStateError::UnknownEntry(entry.0))?;
        let function = match &entry_record.target {
            AwbcEntryTarget::Function { function, .. } => *function,
            AwbcEntryTarget::Routes(_) => return Err(FiberStateError::RouteSelectionRequired),
        };
        Self::for_function_with_instance(
            program,
            AwbcFiberRoot::Entry(entry),
            function,
            instance,
            generation,
            budget_quantum,
        )
    }

    /// Creates a root fiber after the host has selected an exact function from
    /// this entry's closed target inventory.
    pub fn for_entry_target_function(
        program: &AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        generation: u64,
        budget_quantum: u64,
    ) -> Result<Self, FiberStateError> {
        let entry_record = program
            .entries
            .get(entry.index())
            .ok_or(FiberStateError::UnknownEntry(entry.0))?;
        let selected = match &entry_record.target {
            AwbcEntryTarget::Function { function: expected } => *expected == function,
            AwbcEntryTarget::Routes(routes) => routes.iter().any(|route| route.target == function),
        };
        if !selected {
            return Err(FiberStateError::InvalidFrame);
        }
        Self::for_function(
            program,
            AwbcFiberRoot::Entry(entry),
            function,
            generation,
            budget_quantum,
        )
    }

    /// Creates an internal function fiber. Entry target membership is not an
    /// invariant of trait methods, stream transforms, pure calls, or child
    /// functions; external entry/route selection must use
    /// [`Self::for_entry_target_function`].
    pub fn for_function(
        program: &AwbcProgram,
        root: AwbcFiberRoot,
        function: AwbcFunctionId,
        generation: u64,
        budget_quantum: u64,
    ) -> Result<Self, FiberStateError> {
        Self::for_function_with_instance(
            program,
            root,
            function,
            RuntimeFiberInstanceId::from_allocated(std::num::NonZeroU64::MIN),
            generation,
            budget_quantum,
        )
    }

    pub(crate) fn for_function_with_instance(
        program: &AwbcProgram,
        root: AwbcFiberRoot,
        function: AwbcFunctionId,
        instance: RuntimeFiberInstanceId,
        generation: u64,
        budget_quantum: u64,
    ) -> Result<Self, FiberStateError> {
        if root == AwbcFiberRoot::Empty
            || root
                .validate(program)?
                .is_some_and(|expected| expected != function)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let mut next_frame_instance = RuntimeIdCursor::initial();
        let frame_instance = RuntimeFrameInstanceId::from_allocated(
            next_frame_instance.take_next(RuntimeIdNamespace::FrameInstance)?,
        );
        let frame = FiberFrame::new(frame_instance, program, function, None)?;
        Ok(Self {
            instance,
            next_frame_instance,
            generation,
            root,
            cursor: FiberCursor {
                function,
                block: function_record.entry_block,
                instruction_offset: 0,
            },
            frames: vec![frame],
            status: FiberStatus::Running,
            suspension: None,
            terminal: None,
            return_summary: None,
            budget: FiberBudget {
                remaining: budget_quantum,
                quantum: budget_quantum,
            },
            line_cursor: 0,
            streams: program
                .stream_plans
                .iter()
                .enumerate()
                .filter_map(|(index, _)| u32::try_from(index).ok())
                .map(|index| FiberStreamState {
                    plan: AwbcStreamPlanId(index),
                    queue: Vec::new(),
                    closed: false,
                    emitted_count: 0,
                })
                .collect(),
        })
    }

    /// Moves already-admitted positional owners into a frame sealed by a
    /// borrowed preflight. No type checks or other fallible work remains after
    /// the caller transfers `args`.
    pub(crate) fn for_function_with_arguments_prepared(
        program: &AwbcProgram,
        root: AwbcFiberRoot,
        args: Vec<RuntimeValue>,
        prepared: PreparedFunctionInputBinding,
        instance: RuntimeFiberInstanceId,
        generation: u64,
        budget_quantum: u64,
    ) -> Self {
        debug_assert_eq!(prepared.parameter_registers.len(), args.len());
        let mut fiber = Self::for_function_with_instance(
            program,
            root,
            prepared.function,
            instance,
            generation,
            budget_quantum,
        )
        .expect("prepared function input binding validated the AWBC frame");
        fiber.frames[0].type_instantiation = prepared.type_instantiation.map(std::sync::Arc::new);
        let registers = &mut fiber.frames[0].registers;
        for (register, value) in prepared.parameter_registers.into_iter().zip(args) {
            registers[register.index()] = value.into();
        }
        fiber
    }

    /// Creates an internal callback fiber from a fully preflighted zero-arg
    /// callable. All fallible program, body, capture, signature, and frame
    /// checks are performed by `validate_runtime_callable_activation` before
    /// the callback is removed from its owning dialogue.
    pub(crate) fn for_callable_callback_prepared(
        program: &AwbcProgram,
        callable: RuntimeCallableValue,
        prepared: PreparedCallableCallbackActivation,
        instance: RuntimeFiberInstanceId,
        generation: u64,
        budget_quantum: u64,
    ) -> Self {
        let PreparedCallableCallbackActivation {
            function,
            input_layout,
            proof,
        } = prepared;
        let invocation = callable.commit_zero_arg_invocation(proof);
        debug_assert!(matches!(
            invocation.body,
            crate::value::RuntimeCallableBodyReference::Awbc(actual)
                if actual == function
        ));
        debug_assert!(invocation.arguments.is_empty());
        let mut values = invocation.captures;
        values.extend(invocation.arguments);
        Self::for_function_with_arguments_prepared(
            program,
            AwbcFiberRoot::Function(function),
            values,
            input_layout,
            instance,
            generation,
            budget_quantum,
        )
    }

    /// Transactionally binds checked Flow parameter coordinates to the active
    /// Flow frame. This is the sole external Flow ABI path and never resolves
    /// diagnostic parameter names.
    pub(super) fn bind_flow_parameter_coordinates(
        &mut self,
        program: &AwbcProgram,
        bindings: &[RuntimeFlowParameterBinding],
    ) -> Result<(), FiberStateError> {
        let frame = self.active_frame()?;
        let function = program
            .functions
            .get(frame.function.index())
            .ok_or(FiberStateError::UnknownFunction(frame.function.0))?;
        self.bind_active_flow_parameter_coordinates(program, function.signature, bindings)
    }

    /// Transactionally binds arguments to the active function frame.
    pub fn bind_function_arguments(
        &mut self,
        program: &AwbcProgram,
        bindings: &[RuntimeBinding],
    ) -> Result<(), FiberStateError> {
        let frame = self.active_frame()?;
        let function = program
            .functions
            .get(frame.function.index())
            .ok_or(FiberStateError::UnknownFunction(frame.function.0))?;
        self.bind_active_frame_arguments(program, function.signature, bindings)
    }

    /// Transactionally binds positional values to the active function frame.
    ///
    /// This crate-private path is the exact ABI for sealed internal function
    /// activation. It does not resolve parameter names or construct named
    /// bindings; the active frame owns the signature/layout validation and
    /// commits its cloned register vector only after every value is accepted.
    pub(crate) fn bind_function_argument_values(
        &mut self,
        program: &AwbcProgram,
        values: &[RuntimeValue],
    ) -> Result<(), FiberStateError> {
        self.active_frame_mut()?
            .bind_positional_arguments(program, values)
    }

    /// Move-only counterpart used when an external custody packet transfers
    /// affine arguments into this frame. Validation completes before the
    /// register vector is replaced, and no second committed value carrier is
    /// created.
    pub(crate) fn bind_function_argument_values_owned(
        &mut self,
        program: &AwbcProgram,
        values: Vec<RuntimeValue>,
    ) -> Result<(), FiberStateError> {
        self.active_frame_mut()?
            .bind_positional_arguments_owned(program, values)
    }

    /// Atomically removes current parameter storage in sealed positional order,
    /// retaining vacancies and partial owners for the custody reducer.
    pub(crate) fn take_function_argument_storage(
        &mut self,
        program: &AwbcProgram,
    ) -> Result<Vec<RuntimePlaceStorage<RuntimeValue>>, FiberStateError> {
        self.active_frame_mut()?
            .take_positional_argument_storage(program)
    }

    /// Borrows the current function's positional argument slots in the exact
    /// order used by `take_function_argument_storage`, including partial cells.
    pub(crate) fn function_argument_storage<'a>(
        &'a self,
        program: &AwbcProgram,
    ) -> Result<Vec<&'a RuntimePlaceStorage<RuntimeValue>>, FiberStateError> {
        self.active_frame()?.positional_argument_storage(program)
    }

    fn bind_active_frame_arguments(
        &mut self,
        program: &AwbcProgram,
        signature_id: AwbcSignatureId,
        bindings: &[RuntimeBinding],
    ) -> Result<(), FiberStateError> {
        let frame = self.active_frame()?;
        let signature = program
            .signatures
            .get(signature_id.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let layout = program
            .frame_layouts
            .get(frame.layout.index())
            .ok_or(FiberStateError::UnknownFrameLayout(frame.layout.0))?;
        let parameters = layout
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.role == AwbcFrameSlotRole::Parameter)
            .collect::<Vec<_>>();
        if parameters.len() != signature.params.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        if bindings.len() != parameters.len() {
            return Err(FiberStateError::ArgumentCount {
                expected: parameters.len(),
                actual: bindings.len(),
            });
        }

        let parameter_names = parameters
            .iter()
            .map(|(_, slot)| {
                slot.name
                    .and_then(|name| program.strings.get(name.index()).map(String::as_str))
            })
            .collect::<Vec<_>>();
        let named = bindings.iter().any(|binding| {
            parameter_names
                .iter()
                .flatten()
                .any(|name| *name == binding.name)
        });
        let mut assignments = Vec::with_capacity(bindings.len());
        if named {
            let mut used = std::collections::BTreeSet::new();
            for binding in bindings {
                if !used.insert(binding.name.as_str()) {
                    return Err(FiberStateError::DuplicateArgument {
                        name: binding.name.clone(),
                    });
                }
                let Some(position) = parameter_names
                    .iter()
                    .position(|name| name.is_some_and(|name| name == binding.name))
                else {
                    return Err(FiberStateError::UnknownArgument {
                        name: binding.name.clone(),
                    });
                };
                assignments.push((position, &binding.value, binding.name.as_str()));
            }
            assignments.sort_unstable_by_key(|(position, _, _)| *position);
        } else {
            assignments.extend(bindings.iter().enumerate().map(|(position, binding)| {
                let name = parameter_names[position].unwrap_or(binding.name.as_str());
                (position, &binding.value, name)
            }));
        }

        let mut argument_values = vec![None; parameters.len()];
        for &(position, value, _) in &assignments {
            let (register, slot) = parameters[position];
            let expected = signature.params[position];
            if slot.ty != expected {
                return Err(FiberStateError::InvalidFrame);
            }
            argument_values[position] = Some((register, value));
        }
        let references = argument_values
            .iter()
            .map(|value| {
                value
                    .as_ref()
                    .map(|(_, value)| *value)
                    .expect("all ABI inputs assigned")
            })
            .collect::<Vec<_>>();
        let type_instantiation = frame.instantiate_arguments(program, &references)?;
        for (position, value, name) in assignments {
            let expected = signature.params[position];
            if !FiberFrame::value_matches_instantiation(
                program,
                type_instantiation.as_ref(),
                value,
                expected,
            ) {
                return Err(FiberStateError::ArgumentType {
                    name: name.to_owned(),
                    expected: runtime_type_label(program, expected),
                    actual: runtime_value_type_label(value),
                });
            }
        }
        super::vm::validate_function_input_ownership_values(program, frame.function, &references)
            .map_err(|error| FiberStateError::InvalidFunctionInputOwnership {
            reason: error.to_string(),
        })?;
        for (position, value) in references.iter().enumerate() {
            if !value.ownership().permits_copy() {
                return Err(FiberStateError::ArgumentNotCopyable { position });
            }
        }
        let mut register_values = frame.registers.clone();
        for (register, value) in argument_values.into_iter().flatten() {
            register_values[register] = value.clone().into();
        }
        let frame = self.active_frame_mut()?;
        frame.registers = register_values;
        frame.type_instantiation = type_instantiation.map(std::sync::Arc::new);
        Ok(())
    }

    fn bind_active_flow_parameter_coordinates(
        &mut self,
        program: &AwbcProgram,
        signature_id: AwbcSignatureId,
        bindings: &[RuntimeFlowParameterBinding],
    ) -> Result<(), FiberStateError> {
        let frame = self.active_frame()?;
        let signature = program
            .signatures
            .get(signature_id.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let layout = program
            .frame_layouts
            .get(frame.layout.index())
            .ok_or(FiberStateError::UnknownFrameLayout(frame.layout.0))?;
        let parameters = layout
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.role == AwbcFrameSlotRole::Parameter)
            .collect::<Vec<_>>();
        if parameters.len() != signature.params.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        if bindings.len() != parameters.len() {
            return Err(FiberStateError::ArgumentCount {
                expected: parameters.len(),
                actual: bindings.len(),
            });
        }

        let mut used = BTreeSet::new();
        let mut register_values = frame.registers.clone();
        for binding in bindings {
            if !used.insert(binding.parameter) {
                return Err(FiberStateError::DuplicateFlowParameter {
                    parameter: binding.parameter,
                });
            }
            let position =
                binding
                    .parameter
                    .index()
                    .map_err(|_| FiberStateError::UnknownFlowParameter {
                        parameter: binding.parameter,
                    })?;
            let Some((register, slot)) = parameters.get(position).copied() else {
                return Err(FiberStateError::UnknownFlowParameter {
                    parameter: binding.parameter,
                });
            };
            let expected = signature.params[position];
            if slot.ty != expected {
                return Err(FiberStateError::InvalidFrame);
            }
            if !runtime_value_matches_type(program, &binding.value, expected, 0) {
                return Err(FiberStateError::FlowParameterType {
                    parameter: binding.parameter,
                    expected: runtime_type_label(program, expected),
                    actual: runtime_value_type_label(&binding.value),
                });
            }
            register_values[register] = binding.value.clone().into();
        }
        self.active_frame_mut()?.registers = register_values;
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<FiberCheckpoint, FiberStateError> {
        Ok(FiberCheckpoint {
            state: Box::new(AwbcFiberStateSnapshot::from_live(self)?),
        })
    }

    pub fn restore(
        &mut self,
        checkpoint: FiberCheckpoint,
        owner: &RuntimeProgramOwner,
    ) -> Result<(), FiberStateError> {
        self.replace_from_snapshot(*checkpoint.state, owner)
    }

    pub(crate) fn replace_from_snapshot(
        &mut self,
        snapshot: AwbcFiberStateSnapshot,
        owner: &RuntimeProgramOwner,
    ) -> Result<(), FiberStateError> {
        let candidate = snapshot.into_live_for_program(owner)?;
        if let RuntimeProgramOwner::Awbc(program) = owner {
            candidate.validate_for_program(program)?;
        }
        *self = candidate;
        Ok(())
    }

    pub fn validate_for_program(&self, program: &AwbcProgram) -> Result<(), FiberStateError> {
        let expected = self.root.validate(program)?;
        if self.root == AwbcFiberRoot::Empty {
            // An empty executor has no executable cursor or value owner. Do
            // not authenticate its inert cursor against a fictitious function.
            return if self.frames.is_empty()
                && self.status == FiberStatus::Returned
                && self.suspension.is_none()
                && matches!(self.terminal, Some(FiberTerminalValue::Returned(None)))
                && self.return_summary.is_none()
                && self.streams.is_empty()
                && self.line_cursor == 0
                && self.cursor
                    == (FiberCursor {
                        function: AwbcFunctionId::default(),
                        block: AwbcBlockId::default(),
                        instruction_offset: 0,
                    })
            {
                Ok(())
            } else {
                Err(FiberStateError::InvalidFrame)
            };
        }
        if let Some(expected) = expected
            && self
                .frames
                .first()
                .map_or(self.cursor.function, |frame| frame.function)
                != expected
        {
            return Err(FiberStateError::InvalidFrame);
        }
        validate_fiber_terminal_shape(self)?;
        validate_cursor(program, self.cursor)?;
        if let Some(active_frame) = self.frames.last()
            && active_frame.function != self.cursor.function
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let mut frame_instances = BTreeSet::new();
        for (index, frame) in self.frames.iter().enumerate() {
            if !frame_instances.insert(frame.instance) {
                return Err(FiberStateError::InvalidFrame);
            }
            if self
                .next_frame_instance
                .next()
                .is_some_and(|next| frame.instance.get() >= next)
            {
                return Err(FiberStateError::InvalidFrame);
            }
            validate_frame(program, frame, &format!("frames[{index}]"))?;
            if let Some(format) = &frame.format {
                let parked_at = self
                    .frames
                    .get(index + 1)
                    .and_then(|callee| callee.return_to.as_ref())
                    .map_or(self.cursor, |return_to| return_to.cursor);
                if parked_at != format.site {
                    return Err(FiberStateError::InvalidFrame);
                }
                if let Some(callee) = self.frames.get(index + 1) {
                    let continuation = callee.return_to.as_ref().map(|point| &point.continuation);
                    let operand_return = matches!(
                        continuation,
                        Some(FiberReturnContinuation::FormatOperand { site, ordinal })
                            if *site == format.site && *ordinal == format.next_operand
                    );
                    let display_return = matches!(
                        continuation,
                        Some(FiberReturnContinuation::FormatDisplay { site })
                            if *site == format.site
                                && format.next_operand
                                    == format_operand_count_at_site(program, format.site)?
                    );
                    if !operand_return && !display_return {
                        return Err(FiberStateError::InvalidFrame);
                    }
                }
            }
            if index == 0 && frame.return_to.is_some() {
                return Err(FiberStateError::InvalidFrame);
            }
            if let Some(return_to) = &frame.return_to {
                let caller = self
                    .frames
                    .get(index.saturating_sub(1))
                    .ok_or(FiberStateError::InvalidFrame)?;
                validate_return_point(program, caller, frame.function, return_to)?;
            }
        }
        let scope_stacks = self.frames.iter().enumerate().map(|(index, frame)| {
            let cursor = self
                .frames
                .get(index + 1)
                .and_then(|callee| callee.return_to.as_ref())
                .map_or(self.cursor, |return_to| return_to.cursor);
            (cursor, frame.scopes.as_slice())
        });
        program
            .verify_scope_stacks(scope_stacks)
            .map_err(|_| FiberStateError::InvalidFrame)?;
        if matches!(self.status, FiberStatus::Running | FiberStatus::Suspended)
            && self.frames.is_empty()
        {
            return Err(FiberStateError::MissingFrame);
        }
        if let Some(suspension) = &self.suspension {
            if self.status != FiberStatus::Suspended {
                return Err(FiberStateError::InvalidStatus {
                    actual: self.status,
                    expected: FiberStatus::Suspended,
                });
            }
            validate_suspension(program, self, suspension)?;
        }
        if let Some(terminal) = &self.terminal {
            if self
                .frames
                .iter()
                .any(|frame| !frame.format_attempts.is_empty())
            {
                return Err(FiberStateError::InvalidFrame);
            }
            validate_terminal(program, self, terminal)?;
        }
        for (index, stream) in self.streams.iter().enumerate() {
            validate_stream(program, stream, &format!("streams[{index}]"))?;
        }
        Ok(())
    }

    pub fn active_frame(&self) -> Result<&FiberFrame, FiberStateError> {
        self.frames.last().ok_or(FiberStateError::MissingFrame)
    }

    pub fn active_frame_mut(&mut self) -> Result<&mut FiberFrame, FiberStateError> {
        self.frames.last_mut().ok_or(FiberStateError::MissingFrame)
    }

    /// Starts or resumes the formatter staged at the current instruction.
    /// The instruction itself remains the authority for operand order and ABI.
    pub(crate) fn begin_format_content(
        &mut self,
        program: &AwbcProgram,
        context: &RuntimeFormatContext,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let site = self.cursor;
        let operand_count = format_operand_count_at_site(program, site)?;
        let frame = self.active_frame_mut()?;
        if frame.function != site.function {
            return Err(FiberStateError::InvalidFrame);
        }
        if let Some(state) = &frame.format {
            if state.site != site {
                return Err(FiberStateError::InvalidFrame);
            }
            return validate_format_state(program, frame, state);
        }
        frame.format = Some(FiberFormatState {
            site,
            format_context: context.clone(),
            next_operand: 0,
            values: vec![None; operand_count],
            first_recoverable: None,
        });
        Ok(())
    }

    pub(crate) fn format_content_state(&self) -> Result<&FiberFormatState, FiberStateError> {
        let state = self
            .active_frame()?
            .format
            .as_ref()
            .ok_or(FiberStateError::InvalidFrame)?;
        if state.site != self.cursor {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(state)
    }

    /// Takes a fully evaluated formatter after the VM has built its Content.
    pub(crate) fn take_completed_format_content(
        &mut self,
        program: &AwbcProgram,
    ) -> Result<FiberFormatState, FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let site = self.cursor;
        let count = format_operand_count_at_site(program, site)?;
        let frame = self.active_frame()?;
        let state = frame.format.as_ref().ok_or(FiberStateError::InvalidFrame)?;
        validate_format_state(program, frame, state)?;
        if state.site != site || state.next_operand != count {
            return Err(FiberStateError::InvalidFrame);
        }
        self.active_frame_mut()?
            .format
            .take()
            .ok_or(FiberStateError::InvalidFrame)
    }

    /// Converts an evaluation failure into the next formatter operand, or
    /// records a recoverable project DisplayText method failure. Other error
    /// categories never call this boundary.
    /// Any frame carrying a cleanup/defer is left untouched for its VM owner.
    pub(crate) fn recover_format_operand(
        &mut self,
        program: &AwbcProgram,
        reason: String,
    ) -> Result<bool, FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let direct = self
            .frames
            .iter()
            .enumerate()
            .rev()
            .find_map(
                |(index, frame)| match frame.return_to.as_ref()?.continuation {
                    FiberReturnContinuation::FormatOperand { site, ordinal } => {
                        Some((index, site, Some(ordinal)))
                    }
                    FiberReturnContinuation::FormatDisplay { site } => Some((index, site, None)),
                    _ => None,
                },
            );
        if let Some((child_index, site, operand_ordinal)) = direct {
            let child = &self.frames[child_index];
            let caller = self
                .frames
                .get(child_index - 1)
                .ok_or(FiberStateError::InvalidFrame)?;
            validate_return_point(
                program,
                caller,
                child.function,
                child
                    .return_to
                    .as_ref()
                    .ok_or(FiberStateError::InvalidFrame)?,
            )?;
            if self.frames[child_index..]
                .iter()
                .any(FiberFrame::has_pending_cleanup)
            {
                return Ok(false);
            }
            self.frames.truncate(child_index);
            self.cursor = site;
            let state = self
                .active_frame_mut()?
                .format
                .as_mut()
                .ok_or(FiberStateError::InvalidFrame)?;
            if state.first_recoverable.is_none() {
                state.first_recoverable = Some(reason);
            }
            if let Some(ordinal) = operand_ordinal {
                state.next_operand = ordinal + 1;
            }
            return Ok(true);
        }

        self.recover_format_attempt_operand(program, reason)
    }

    pub fn safe_point(
        &self,
        resume: Option<AwbcResumePointId>,
    ) -> Result<FiberSafePoint, FiberStateError> {
        let frame = self.active_frame()?;
        Ok(FiberSafePoint {
            generation: self.generation,
            cursor: self.cursor,
            frame_layout: frame.layout,
            resume,
        })
    }

    /// Commits one externally handled yielding instruction after confirming
    /// that the observed cursor is still the fiber's exact running position.
    ///
    /// All validation, including the checked cursor advance, happens before
    /// the fiber is mutated. A stale observation, invalid running state, or
    /// offset overflow therefore leaves the fiber unchanged.
    pub fn commit_yielded_instruction(
        &mut self,
        observed_cursor: FiberCursor,
    ) -> Result<(), FiberStateError> {
        let prepared = self.validate_yielded_instruction(observed_cursor)?;
        self.commit_yielded_instruction_prepared(prepared);
        Ok(())
    }

    pub(crate) fn validate_yielded_instruction(
        &self,
        observed_cursor: FiberCursor,
    ) -> Result<PreparedYieldedInstruction, FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let active_frame = self.active_frame()?;
        let current_cursor = self.cursor;
        if current_cursor != observed_cursor {
            return Err(FiberStateError::StaleCursor {
                observed: observed_cursor,
                current: current_cursor,
            });
        }
        if active_frame.function != current_cursor.function {
            return Err(FiberStateError::InvalidFrame);
        }
        let next_offset = current_cursor.instruction_offset.checked_add(1).ok_or(
            FiberStateError::InstructionOffsetOverflow {
                cursor: current_cursor,
            },
        )?;
        Ok(PreparedYieldedInstruction {
            fiber: self.instance,
            frame: active_frame.instance,
            cursor: current_cursor,
            next_offset,
        })
    }

    pub(crate) fn commit_yielded_instruction_prepared(
        &mut self,
        prepared: PreparedYieldedInstruction,
    ) {
        assert_eq!(self.instance, prepared.fiber);
        assert_eq!(self.cursor, prepared.cursor);
        let frame = self
            .active_frame()
            .expect("prepared yielded instruction retains its active frame");
        assert_eq!(frame.instance, prepared.frame);
        self.cursor.instruction_offset = prepared.next_offset;
    }

    pub(crate) fn validate_yielded_register_write(
        &self,
        observed_cursor: FiberCursor,
        register: AwbcRegisterId,
    ) -> Result<PreparedYieldedRegisterWrite, FiberStateError> {
        let instruction = self.validate_yielded_instruction(observed_cursor)?;
        let frame = self.active_frame()?;
        let slot =
            frame
                .registers
                .get(register.index())
                .ok_or(FiberStateError::RegisterOutOfBounds {
                    register: register.0,
                    layout: frame.layout.0,
                })?;
        if !slot.is_vacant() {
            return Err(FiberStateError::RegisterAlreadyInitialized {
                register: register.0,
                layout: frame.layout.0,
            });
        }
        Ok(PreparedYieldedRegisterWrite {
            instruction,
            register,
        })
    }

    pub(crate) fn commit_yielded_register_write_prepared(
        &mut self,
        prepared: PreparedYieldedRegisterWrite,
        value: RuntimeValue,
    ) {
        assert_eq!(self.instance, prepared.instruction.fiber);
        assert_eq!(self.cursor, prepared.instruction.cursor);
        let frame = self
            .active_frame_mut()
            .expect("prepared yielded register write retains its active frame");
        assert_eq!(frame.instance, prepared.instruction.frame);
        let slot = frame
            .registers
            .get_mut(prepared.register.index())
            .expect("prepared yielded register write retains its destination");
        assert!(
            slot.is_vacant(),
            "prepared destination register remains vacant"
        );
        *slot = value.into();
        self.cursor.instruction_offset = prepared.instruction.next_offset;
    }

    pub(crate) fn inspect_yielded_operand_restore(
        &self,
        program: &AwbcProgram,
        observed_cursor: FiberCursor,
        operands: &[(AwbcRegisterId, &RuntimeValue)],
    ) -> Result<PreparedYieldedOperandRestore, FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        if self.cursor != observed_cursor {
            return Err(FiberStateError::StaleCursor {
                observed: observed_cursor,
                current: self.cursor,
            });
        }
        let frame = self.active_frame()?;
        if frame.function != observed_cursor.function {
            return Err(FiberStateError::InvalidFrame);
        }
        let expected_registers = match instruction_at_site(program, observed_cursor)? {
            AwbcInstruction::ExecuteLineOperation {
                operation, args, ..
            } => match program.line_operations.get(operation.index()) {
                Some(super::schema::AwbcLineOperation::ActorLook { .. }) => {
                    args.get(1..).ok_or(FiberStateError::InvalidFrame)?.to_vec()
                }
                Some(_) => args.clone(),
                None => return Err(FiberStateError::InvalidFrame),
            },
            AwbcInstruction::RegisterDefer { captures, .. } => captures.clone(),
            AwbcInstruction::CommitDialogueResult { source } => vec![*source],
            _ => return Err(FiberStateError::InvalidFrame),
        };
        if expected_registers.len() != operands.len()
            || expected_registers
                .iter()
                .zip(operands)
                .any(|(expected, (actual, _))| expected != actual)
        {
            return Err(FiberStateError::InvalidFrame);
        }

        let layout = program
            .frame_layouts
            .get(frame.layout.index())
            .ok_or(FiberStateError::UnknownFrameLayout(frame.layout.0))?;
        let mut seen = BTreeSet::new();
        let mut types = Vec::with_capacity(operands.len());
        for (register, value) in operands {
            if !seen.insert(*register) {
                return Err(FiberStateError::InvalidFrame);
            }
            let live_slot = frame.registers.get(register.index()).ok_or(
                FiberStateError::RegisterOutOfBounds {
                    register: register.0,
                    layout: frame.layout.0,
                },
            )?;
            if !live_slot.is_vacant() {
                return Err(FiberStateError::RegisterAlreadyInitialized {
                    register: register.0,
                    layout: frame.layout.0,
                });
            }
            let layout_slot =
                layout
                    .slots
                    .get(register.index())
                    .ok_or(FiberStateError::RegisterOutOfBounds {
                        register: register.0,
                        layout: frame.layout.0,
                    })?;
            if !frame.value_matches_type(program, value, layout_slot.ty) {
                return Err(FiberStateError::InvalidRuntimeValue {
                    path: format!("yielded operand register {}", register.0),
                    reason: "value does not match the sealed register type".to_owned(),
                });
            }
            types.push(layout_slot.ty);
        }

        Ok(PreparedYieldedOperandRestore {
            fiber: self.instance,
            frame: frame.instance,
            cursor: observed_cursor,
            registers: expected_registers.into_boxed_slice(),
            types: types.into_boxed_slice(),
        })
    }

    pub(crate) fn restore_yielded_operands_prepared(
        &mut self,
        program: &AwbcProgram,
        prepared: PreparedYieldedOperandRestore,
        operands: Vec<(AwbcRegisterId, RuntimeValue)>,
    ) {
        assert_eq!(self.instance, prepared.fiber);
        assert_eq!(self.cursor, prepared.cursor);
        let frame = self
            .active_frame_mut()
            .expect("prepared operand restore retains its active frame");
        assert_eq!(frame.instance, prepared.frame);
        assert_eq!(operands.len(), prepared.registers.len());
        assert_eq!(operands.len(), prepared.types.len());
        for (((register, value), expected_register), expected_type) in operands
            .into_iter()
            .zip(prepared.registers.iter().copied())
            .zip(prepared.types.iter().copied())
        {
            assert_eq!(register, expected_register);
            assert!(frame.value_matches_type(program, &value, expected_type));
            let slot = frame
                .registers
                .get_mut(register.index())
                .expect("prepared operand restore retains its source register");
            assert!(slot.is_vacant(), "prepared source register remains vacant");
            *slot = value.into();
        }
    }

    pub fn consume_budget(&mut self, units: u64) -> bool {
        if units > self.budget.remaining {
            return false;
        }
        self.budget.remaining -= units;
        true
    }

    pub fn replenish_budget(&mut self) {
        self.budget.remaining = self.budget.quantum;
    }

    pub fn suspend(&mut self, suspension: FiberSuspension) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        self.status = FiberStatus::Suspended;
        self.suspension = Some(suspension);
        Ok(())
    }

    /// Applies a verified resume point after the host/VM has materialized results.
    pub fn resume_at(
        &mut self,
        program: &AwbcProgram,
        resume: AwbcResumePointId,
    ) -> Result<(), FiberStateError> {
        let prepared = self.validate_resume_at(program, resume)?;
        self.resume_at_prepared(prepared);
        Ok(())
    }

    /// Validates a declared resume point without changing the suspended fiber.
    pub(crate) fn validate_resume_at(
        &self,
        program: &AwbcProgram,
        resume: AwbcResumePointId,
    ) -> Result<PreparedFiberResume, FiberStateError> {
        self.require_status(FiberStatus::Suspended)?;
        if self
            .suspension
            .as_ref()
            .and_then(FiberSuspension::declared_resume)
            != Some(resume)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let point = program
            .resume_points
            .get(resume.index())
            .ok_or(FiberStateError::UnknownResumePoint(resume.0))?;
        let frame = self.active_frame()?;
        if point.function != frame.function {
            return Err(FiberStateError::ResumeFunctionMismatch {
                resume: resume.0,
                actual: point.function.0,
                expected: frame.function.0,
            });
        }
        if point.frame_layout != frame.layout {
            return Err(FiberStateError::ResumeLayoutMismatch {
                resume: resume.0,
                actual: point.frame_layout.0,
                expected: frame.layout.0,
            });
        }
        Ok(PreparedFiberResume {
            fiber: self.instance,
            frame: frame.instance,
            current_cursor: self.cursor,
            resume,
            target: FiberCursor {
                function: point.function,
                block: point.block,
                instruction_offset: 0,
            },
        })
    }

    /// Commits a resume point already validated against this exact dynamic
    /// fiber/frame state. No data-dependent failure remains after ownership
    /// moves into the resumed frame.
    pub(crate) fn resume_at_prepared(&mut self, prepared: PreparedFiberResume) {
        assert_eq!(self.instance, prepared.fiber);
        assert_eq!(self.cursor, prepared.current_cursor);
        assert!(matches!(self.status, FiberStatus::Suspended));
        assert_eq!(
            self.suspension
                .as_ref()
                .and_then(FiberSuspension::declared_resume),
            Some(prepared.resume)
        );
        assert_eq!(
            self.active_frame()
                .expect("prepared resume retains its frame")
                .instance,
            prepared.frame
        );
        self.cursor = prepared.target;
        self.status = FiberStatus::Running;
        self.suspension = None;
    }

    /// Selects the verified Progress continuation retained by an Await
    /// suspension, then resumes through the ordinary declared-point path.
    pub fn resume_await_observer_at(
        &mut self,
        program: &AwbcProgram,
        resume: AwbcResumePointId,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Suspended)?;
        let suspension = self
            .suspension
            .as_ref()
            .ok_or(FiberStateError::InvalidFrame)?;
        let FiberSuspensionReason::Await {
            target,
            binding,
            observer: Some(observer),
        } = &suspension.reason
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        if observer.resume != resume {
            return Err(FiberStateError::InvalidFrame);
        }
        if !matches!(
            self.active_frame()?.register(observer.destination),
            Ok(RuntimeValue::Progress(_))
        ) {
            return Err(FiberStateError::InvalidRuntimeValue {
                path: "suspension.await.observer.destination".to_owned(),
                reason: "Await observer resume requires its published Progress value".to_owned(),
            });
        }
        validate_await_suspension(program, self.active_frame()?, target, *binding)?;

        let previous_resume = suspension.resume;
        self.suspension
            .as_mut()
            .expect("validated suspension exists")
            .resume = FiberResumeTarget::Declared(resume);
        let prepared = match self.validate_resume_at(program, resume) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.suspension
                    .as_mut()
                    .expect("validated suspension exists")
                    .resume = previous_resume;
                return Err(error);
            }
        };
        let reason = std::mem::replace(
            &mut self
                .suspension
                .as_mut()
                .expect("validated suspension exists")
                .reason,
            FiberSuspensionReason::BudgetYield,
        );
        let FiberSuspensionReason::Await {
            target: FiberAwaitTarget::Need { need, handle, .. },
            ..
        } = reason
        else {
            unreachable!("validated observer suspension owns a Need")
        };
        self.resume_at_prepared(prepared);
        let frame = self
            .active_frame_mut()
            .expect("prepared resume retains its frame");
        let slot = frame
            .registers
            .get_mut(handle.index())
            .expect("validated Await handle register exists");
        assert!(
            slot.is_vacant(),
            "validated Await handle register stays vacant"
        );
        *slot = RuntimeValue::NeedHandle(need).into();
        Ok(())
    }

    /// Resumes a budget yield at its declared or exact preemption target.
    pub fn resume_budget_yield(&mut self, program: &AwbcProgram) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Suspended)?;
        let suspension = self
            .suspension
            .as_ref()
            .ok_or(FiberStateError::InvalidFrame)?;
        if suspension.reason != FiberSuspensionReason::BudgetYield {
            return Err(FiberStateError::InvalidFrame);
        }
        match suspension.resume {
            FiberResumeTarget::Declared(resume) => self.resume_at(program, resume),
            FiberResumeTarget::Exact(cursor) => {
                validate_cursor(program, cursor)?;
                if self.active_frame()?.function != cursor.function {
                    return Err(FiberStateError::InvalidFrame);
                }
                self.cursor = cursor;
                self.status = FiberStatus::Running;
                self.suspension = None;
                Ok(())
            }
        }
    }

    pub fn push_call_frame(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        return_to: AwbcResumePointId,
        destination: Option<AwbcRegisterId>,
    ) -> Result<(), FiberStateError> {
        self.push_call_frame_with_args(program, function, return_to, destination, &[])
    }

    pub fn push_call_frame_with_args(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        return_to: AwbcResumePointId,
        destination: Option<AwbcRegisterId>,
        args: &[RuntimeValue],
    ) -> Result<(), FiberStateError> {
        let caller = self.active_frame()?;
        let point = validate_resume_point(program, caller, return_to)?;
        self.push_call_frame_at(
            program,
            function,
            FiberReturnPoint::ordinary(
                FiberCursor {
                    function: point.function,
                    block: point.block,
                    instruction_offset: 0,
                },
                destination,
            ),
            args,
        )
    }

    /// Pushes a call frame by transferring its already evaluated arguments.
    ///
    /// Unlike the borrowed compatibility path, this does not clone captured
    /// affine values into the callee frame.
    pub fn push_call_frame_with_owned_args(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        return_to: AwbcResumePointId,
        destination: Option<AwbcRegisterId>,
        args: Vec<RuntimeValue>,
    ) -> Result<(), FiberStateError> {
        let caller = self.active_frame()?;
        let point = validate_resume_point(program, caller, return_to)?;
        self.push_call_frame_at_owned(
            program,
            function,
            FiberReturnPoint::ordinary(
                FiberCursor {
                    function: point.function,
                    block: point.block,
                    instruction_offset: 0,
                },
                destination,
            ),
            args,
        )
    }

    /// Pushes a function frame with an exact caller continuation.
    ///
    /// The continuation and all positional arguments are validated before the
    /// live fiber is mutated, so malformed function values cannot leave a
    /// partially entered frame behind.
    pub fn push_call_frame_at(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        return_to: FiberReturnPoint,
        args: &[RuntimeValue],
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        match return_to.continuation {
            FiberReturnContinuation::FormatOperand { .. } if self.cursor != return_to.cursor => {
                return Err(FiberStateError::InvalidFrame);
            }
            FiberReturnContinuation::FormatDisplay { site }
                if self.cursor != site || return_to.cursor != site =>
            {
                return Err(FiberStateError::InvalidFrame);
            }
            FiberReturnContinuation::InstructionCall { site } if self.cursor != site => {
                return Err(FiberStateError::InvalidFrame);
            }
            FiberReturnContinuation::ContextCallbackDefault { site, .. }
            | FiberReturnContinuation::ContextCallbackInvoke { site, .. }
                if self.cursor != site || return_to.cursor != site =>
            {
                return Err(FiberStateError::InvalidFrame);
            }
            _ => {}
        }
        validate_return_point(program, self.active_frame()?, function, &return_to)?;
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let mut next_frame_instance = self.next_frame_instance;
        let frame_instance = RuntimeFrameInstanceId::from_allocated(
            next_frame_instance.take_next(RuntimeIdNamespace::FrameInstance)?,
        );
        let mut frame = FiberFrame::new(frame_instance, program, function, Some(return_to))?;
        frame.bind_positional_arguments(program, args)?;
        self.next_frame_instance = next_frame_instance;
        self.frames.push(frame);
        self.cursor = FiberCursor {
            function,
            block: function_record.entry_block,
            instruction_offset: 0,
        };
        Ok(())
    }

    /// Pushes a function frame with owned positional arguments.
    pub(crate) fn push_call_frame_at_owned(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        return_to: FiberReturnPoint,
        args: Vec<RuntimeValue>,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        match return_to.continuation {
            FiberReturnContinuation::FormatOperand { .. } if self.cursor != return_to.cursor => {
                return Err(FiberStateError::InvalidFrame);
            }
            FiberReturnContinuation::FormatDisplay { site }
                if self.cursor != site || return_to.cursor != site =>
            {
                return Err(FiberStateError::InvalidFrame);
            }
            FiberReturnContinuation::InstructionCall { site } if self.cursor != site => {
                return Err(FiberStateError::InvalidFrame);
            }
            FiberReturnContinuation::ContextCallbackDefault { site, .. }
            | FiberReturnContinuation::ContextCallbackInvoke { site, .. }
                if self.cursor != site || return_to.cursor != site =>
            {
                return Err(FiberStateError::InvalidFrame);
            }
            _ => {}
        }
        validate_return_point(program, self.active_frame()?, function, &return_to)?;
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let mut next_frame_instance = self.next_frame_instance;
        let frame_instance = RuntimeFrameInstanceId::from_allocated(
            next_frame_instance.take_next(RuntimeIdNamespace::FrameInstance)?,
        );
        let mut frame = FiberFrame::new(frame_instance, program, function, Some(return_to))?;
        frame.bind_positional_arguments_owned(program, args)?;
        self.next_frame_instance = next_frame_instance;
        self.frames.push(frame);
        self.cursor = FiberCursor {
            function,
            block: function_record.entry_block,
            instruction_offset: 0,
        };
        Ok(())
    }

    /// Enters a callee with an explicit typed return continuation. ProjectCall
    /// stages use this single frame-owned union so default/target values remain
    /// part of the persisted call boundary rather than a parallel side table.
    pub(crate) fn push_call_frame_with_continuation(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        return_to: FiberReturnPoint,
        args: &[RuntimeValue],
    ) -> Result<(), FiberStateError> {
        self.push_call_frame_at(program, function, return_to, args)
    }

    /// Enters a callee while transferring its arguments into the new frame.
    pub(crate) fn push_call_frame_with_owned_continuation(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        return_to: FiberReturnPoint,
        args: Vec<RuntimeValue>,
    ) -> Result<(), FiberStateError> {
        self.push_call_frame_at_owned(program, function, return_to, args)
    }

    /// Replaces the active frame with a tail-called function while preserving
    /// the caller return point.
    pub fn replace_active_function(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        args: &[RuntimeValue],
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let mut next_frame_instance = self.next_frame_instance;
        let frame_instance = RuntimeFrameInstanceId::from_allocated(
            next_frame_instance.take_next(RuntimeIdNamespace::FrameInstance)?,
        );
        let mut frame = FiberFrame::new(frame_instance, program, function, None)?;
        frame.bind_positional_arguments(program, args)?;
        frame.return_to = self.active_frame_mut()?.return_to.take();
        self.next_frame_instance = next_frame_instance;
        *self.active_frame_mut()? = frame;
        self.cursor = FiberCursor {
            function,
            block: function_record.entry_block,
            instruction_offset: 0,
        };
        Ok(())
    }

    /// Replaces the entire active call stack with a new root Flow function.
    ///
    /// A Goto is a nonlocal Flow transfer, so preserving any nested return
    /// point would incorrectly re-enter a ProjectCall default/target stage.
    /// Cleanup ownership is drained by the VM before this method is called;
    /// this method only constructs the validated root frame transactionally.
    pub fn replace_root_function(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        args: &[RuntimeValue],
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let mut next_frame_instance = self.next_frame_instance;
        let frame_instance = RuntimeFrameInstanceId::from_allocated(
            next_frame_instance.take_next(RuntimeIdNamespace::FrameInstance)?,
        );
        let mut frame = FiberFrame::new(frame_instance, program, function, None)?;
        frame.bind_positional_arguments(program, args)?;
        self.next_frame_instance = next_frame_instance;
        self.frames.clear();
        self.frames.push(frame);
        self.cursor = FiberCursor {
            function,
            block: function_record.entry_block,
            instruction_offset: 0,
        };
        Ok(())
    }

    /// Replaces the call stack while transferring already evaluated Flow
    /// arguments into the new root frame.
    pub(crate) fn replace_root_function_owned(
        &mut self,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        args: Vec<RuntimeValue>,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let mut next_frame_instance = self.next_frame_instance;
        let frame_instance = RuntimeFrameInstanceId::from_allocated(
            next_frame_instance.take_next(RuntimeIdNamespace::FrameInstance)?,
        );
        let mut frame = FiberFrame::new(frame_instance, program, function, None)?;
        frame.bind_positional_arguments_owned(program, args)?;
        self.next_frame_instance = next_frame_instance;
        self.frames.clear();
        self.frames.push(frame);
        self.cursor = FiberCursor {
            function,
            block: function_record.entry_block,
            instruction_offset: 0,
        };
        Ok(())
    }

    pub fn pop_call_frame(
        &mut self,
        program: &AwbcProgram,
    ) -> Result<Option<FiberReturnPoint>, FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let frame = self.frames.last().ok_or(FiberStateError::MissingFrame)?;
        let Some(return_to) = frame.return_to.as_ref() else {
            return Ok(None);
        };
        let returning_function = frame.function;
        let caller = self
            .frames
            .get(self.frames.len().saturating_sub(2))
            .ok_or(FiberStateError::MissingFrame)?;
        validate_return_point(program, caller, returning_function, &return_to)?;
        let popped = self.frames.pop().ok_or(FiberStateError::MissingFrame)?;
        let return_to = popped.return_to.ok_or(FiberStateError::InvalidFrame)?;
        self.cursor = return_to.cursor;
        Ok(Some(return_to))
    }

    /// Completes either a nested call or the root function.
    ///
    /// Shape and destination checks happen before the callee frame is removed so
    /// an invalid compiled/VM return cannot partially mutate the fiber.
    pub fn finish_return(
        &mut self,
        program: &AwbcProgram,
        value: Option<RuntimeValue>,
    ) -> Result<bool, FiberStateError> {
        self.finish_return_with_continuation(program, value)
            .map(|(return_to, _)| return_to.is_none())
    }

    /// Completes a return and moves any continuation payload to the VM caller.
    /// The returned value is present only for project/context continuations
    /// whose completion logic lives outside the fiber.
    pub(crate) fn finish_return_with_continuation(
        &mut self,
        program: &AwbcProgram,
        value: Option<RuntimeValue>,
    ) -> Result<(Option<FiberReturnPoint>, Option<RuntimeValue>), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        if self.frames.len() == 1 {
            let frame = self.active_frame()?;
            let signature = program
                .functions
                .get(frame.function.index())
                .and_then(|function| program.signatures.get(function.signature.index()))
                .ok_or(FiberStateError::InvalidFrame)?;
            if let Some(value) = &value
                && !signature
                    .result
                    .is_some_and(|expected| frame.value_matches_type(program, value, expected))
            {
                return Err(FiberStateError::ReturnValueMismatch);
            }
            self.mark_returned(value)?;
            return Ok((None, None));
        }
        let returning_frame = self.frames.last().ok_or(FiberStateError::MissingFrame)?;
        let return_to = returning_frame
            .return_to
            .as_ref()
            .ok_or(FiberStateError::InvalidFrame)?;
        let caller_frame = self
            .frames
            .get(self.frames.len() - 2)
            .ok_or(FiberStateError::MissingFrame)?;
        if matches!(
            &return_to.continuation,
            FiberReturnContinuation::InstructionCall { .. }
        ) {
            validate_return_point(program, caller_frame, returning_frame.function, &return_to)?;
        }
        let signature = program
            .functions
            .get(returning_frame.function.index())
            .and_then(|function| program.signatures.get(function.signature.index()))
            .ok_or(FiberStateError::InvalidFrame)?;
        let return_value = match (signature.result, value) {
            (Some(expected), Some(value))
                if returning_frame.value_matches_type(program, &value, expected) =>
            {
                Some(value)
            }
            (None, None) if return_to.destination.is_some() => Some(RuntimeValue::Unit),
            (None, None) => None,
            _ => return Err(FiberStateError::ReturnValueMismatch),
        };
        if matches!(
            &return_to.continuation,
            FiberReturnContinuation::FormatOperand { .. }
                | FiberReturnContinuation::FormatDisplay { .. }
        ) && return_value.is_none()
        {
            return Err(FiberStateError::ReturnValueMismatch);
        }
        if let (Some(destination), Some(value)) = (return_to.destination, return_value.as_ref()) {
            if destination.index() >= caller_frame.registers.len() {
                return Err(FiberStateError::RegisterOutOfBounds {
                    register: destination.0,
                    layout: caller_frame.layout.0,
                });
            }
            let destination_type = program
                .frame_layouts
                .get(caller_frame.layout.index())
                .and_then(|layout| layout.slots.get(destination.index()))
                .map(|slot| slot.ty)
                .ok_or(FiberStateError::InvalidFrame)?;
            if !caller_frame.value_matches_type(program, value, destination_type) {
                return Err(FiberStateError::ReturnValueMismatch);
            }
        }
        let receiver_update = instruction_call_receiver_update(
            program,
            caller_frame,
            returning_frame,
            &return_to.continuation,
        )?;
        let popped = self
            .pop_call_frame(program)?
            .ok_or(FiberStateError::InvalidFrame)?;
        let external = matches!(
            &popped.continuation,
            FiberReturnContinuation::ProjectCallDefault { .. }
                | FiberReturnContinuation::ProjectCallTarget { .. }
                | FiberReturnContinuation::ApplyGroupDefault { .. }
                | FiberReturnContinuation::ContextCallbackDefault { .. }
                | FiberReturnContinuation::ContextCallbackInvoke { .. }
        );
        let format_operand_ordinal = match &popped.continuation {
            FiberReturnContinuation::FormatOperand { ordinal, .. } => Some(*ordinal),
            _ => None,
        };
        if let Some(ordinal) = format_operand_ordinal {
            let state = self
                .active_frame_mut()?
                .format
                .as_mut()
                .ok_or(FiberStateError::InvalidFrame)?;
            state.values[ordinal] = return_value;
            state.next_operand = ordinal + 1;
            return Ok((Some(popped), None));
        }
        if let Some((destination, value)) = receiver_update {
            self.active_frame_mut()?.set_register(destination, value)?;
        }
        if external {
            return Ok((Some(popped), return_value));
        }
        if let (Some(destination), Some(value)) = (popped.destination, return_value) {
            self.active_frame_mut()?.set_register(destination, value)?;
        }
        Ok((Some(popped), None))
    }

    pub fn mark_returned(&mut self, value: Option<RuntimeValue>) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        self.discard_format_attempts();
        self.status = FiberStatus::Returned;
        self.suspension = None;
        self.return_summary = value.as_ref().map(crate::value::runtime_value_label);
        self.terminal = Some(FiberTerminalValue::Returned(value));
        Ok(())
    }

    pub fn mark_dialogue_result_selected(
        &mut self,
        value: RuntimeValue,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        self.discard_format_attempts();
        self.status = FiberStatus::Returned;
        self.suspension = None;
        self.return_summary = None;
        self.terminal = Some(FiberTerminalValue::DialogueResultSelected(value));
        Ok(())
    }

    pub(super) fn mark_cancelled(&mut self) {
        self.discard_format_attempts();
        self.status = FiberStatus::Cancelled;
        self.suspension = None;
        self.return_summary = None;
        self.terminal = Some(FiberTerminalValue::Cancelled);
    }

    pub fn mark_trapped(&mut self, trap: FiberTrap) {
        if matches!(
            self.status,
            FiberStatus::Returned | FiberStatus::Cancelled | FiberStatus::Trapped
        ) {
            return;
        }
        self.discard_format_attempts();
        self.status = FiberStatus::Trapped;
        self.suspension = None;
        self.return_summary = None;
        self.terminal = Some(FiberTerminalValue::Trapped(trap));
    }

    /// Detaches all registered cleanups in whole-stack unwind order.
    ///
    /// Frames unwind from callee to caller. Each frame first drains lexical
    /// scopes from innermost to outermost and then drains frame-root cleanups.
    /// Entries are removed while collecting them, so a later terminal signal
    /// cannot execute the same cleanup twice.
    pub(super) fn take_unwind_cleanups(&mut self) -> Vec<FiberScopeCleanup> {
        let mut cleanups = Vec::new();
        for frame in self.frames.iter_mut().rev() {
            for scope in frame.scopes.iter_mut().rev() {
                while let Some(cleanup) = scope.cleanups.pop() {
                    cleanups.push(cleanup);
                }
            }
            while let Some(cleanup) = frame.root_cleanups.pop() {
                cleanups.push(cleanup);
            }
        }
        cleanups
    }

    pub(super) fn take_active_frame_cleanups(
        &mut self,
    ) -> Result<Vec<FiberScopeCleanup>, FiberStateError> {
        let frame = self.active_frame_mut()?;
        let mut cleanups = Vec::new();
        for scope in frame.scopes.iter_mut().rev() {
            while let Some(cleanup) = scope.cleanups.pop() {
                cleanups.push(cleanup);
            }
        }
        while let Some(cleanup) = frame.root_cleanups.pop() {
            cleanups.push(cleanup);
        }
        Ok(cleanups)
    }

    fn require_status(&self, expected: FiberStatus) -> Result<(), FiberStateError> {
        if self.status == expected {
            Ok(())
        } else {
            Err(FiberStateError::InvalidStatus {
                actual: self.status,
                expected,
            })
        }
    }
}

fn validate_fiber_terminal_shape(state: &FiberState) -> Result<(), FiberStateError> {
    match state.status {
        FiberStatus::Running => {
            if state.suspension.is_some()
                || state.terminal.is_some()
                || state.return_summary.is_some()
            {
                return Err(FiberStateError::InvalidStatus {
                    actual: state.status,
                    expected: FiberStatus::Running,
                });
            }
        }
        FiberStatus::Suspended => {
            if state.suspension.is_none()
                || state.terminal.is_some()
                || state.return_summary.is_some()
            {
                return Err(FiberStateError::InvalidStatus {
                    actual: state.status,
                    expected: FiberStatus::Suspended,
                });
            }
        }
        FiberStatus::Returned => {
            let terminal_matches = match state.terminal.as_ref() {
                Some(FiberTerminalValue::Returned(Some(value))) => {
                    let expected = crate::value::runtime_value_label(value);
                    state.return_summary.as_deref() == Some(expected.as_str())
                }
                Some(FiberTerminalValue::Returned(None)) => true,
                Some(FiberTerminalValue::DialogueResultSelected(_)) => {
                    state.return_summary.is_none()
                }
                _ => false,
            };
            if state.suspension.is_some() || !terminal_matches {
                return Err(FiberStateError::InvalidStatus {
                    actual: state.status,
                    expected: state.status,
                });
            }
        }
        FiberStatus::Cancelled => {
            if state.suspension.is_some()
                || state.return_summary.is_some()
                || !matches!(state.terminal.as_ref(), Some(FiberTerminalValue::Cancelled))
            {
                return Err(FiberStateError::InvalidStatus {
                    actual: state.status,
                    expected: state.status,
                });
            }
        }
        FiberStatus::Trapped => {
            if state.suspension.is_some()
                || state.return_summary.is_some()
                || !matches!(
                    state.terminal.as_ref(),
                    Some(FiberTerminalValue::Trapped(_))
                )
            {
                return Err(FiberStateError::InvalidStatus {
                    actual: state.status,
                    expected: state.status,
                });
            }
        }
    }
    Ok(())
}

fn validate_cursor(program: &AwbcProgram, cursor: FiberCursor) -> Result<(), FiberStateError> {
    let function = program
        .functions
        .get(cursor.function.index())
        .ok_or(FiberStateError::UnknownFunction(cursor.function.0))?;
    if !function_owns_block(function, cursor.block) {
        return Err(FiberStateError::InvalidFrame);
    }
    let block = program
        .blocks
        .get(cursor.block.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if block.owner != cursor.function || cursor.instruction_offset > block.instructions.len {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn instruction_at_site(
    program: &AwbcProgram,
    site: FiberCursor,
) -> Result<&AwbcInstruction, FiberStateError> {
    validate_cursor(program, site)?;
    let block = &program.blocks[site.block.index()];
    if site.instruction_offset >= block.instructions.len {
        return Err(FiberStateError::InvalidFrame);
    }
    let instruction = block
        .instructions
        .start
        .checked_add(site.instruction_offset)
        .and_then(|index| program.instructions.get(index as usize))
        .ok_or(FiberStateError::InvalidFrame)?;
    Ok(instruction)
}

fn format_operands_at_site(
    program: &AwbcProgram,
    site: FiberCursor,
) -> Result<&[super::schema::AwbcFormatOperand], FiberStateError> {
    let AwbcInstruction::FormatContent { operands, .. } = instruction_at_site(program, site)?
    else {
        return Err(FiberStateError::InvalidFrame);
    };
    Ok(operands)
}

fn format_operand_count_at_site(
    program: &AwbcProgram,
    site: FiberCursor,
) -> Result<usize, FiberStateError> {
    if let Some((_, operands)) = format::format_attempt_operands_at_site(program, site)? {
        Ok(operands.len())
    } else {
        Ok(format_operands_at_site(program, site)?.len())
    }
}

fn format_operand_type_at_site(
    program: &AwbcProgram,
    site: FiberCursor,
    ordinal: usize,
) -> Result<AwbcTypeId, FiberStateError> {
    if let Some((_, operands)) = format::format_attempt_operands_at_site(program, site)? {
        return operands
            .get(ordinal)
            .map(|operand| operand.ty)
            .ok_or(FiberStateError::InvalidFrame);
    }
    let operand = format_operands_at_site(program, site)?
        .get(ordinal)
        .ok_or(FiberStateError::InvalidFrame)?;
    program
        .functions
        .get(operand.function.index())
        .and_then(|function| program.signatures.get(function.signature.index()))
        .and_then(|signature| signature.result)
        .ok_or(FiberStateError::InvalidFrame)
}

fn format_project_method_at_site(
    program: &AwbcProgram,
    site: FiberCursor,
) -> Result<Option<(&super::schema::AwbcTraitMethod, AwbcRegisterId, AwbcTypeId)>, FiberStateError>
{
    let AwbcInstruction::FormatContent {
        project_method,
        project_result,
        ..
    } = instruction_at_site(program, site)?
    else {
        return Err(FiberStateError::InvalidFrame);
    };
    match (project_method, project_result) {
        (None, None) => Ok(None),
        (Some(method), Some(result)) => {
            let method = program
                .trait_methods
                .get(method.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            let function = program
                .functions
                .get(method.function.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            let signature = program
                .signatures
                .get(function.signature.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            let result_type = signature.result.ok_or(FiberStateError::InvalidFrame)?;
            Ok(Some((method, *result, result_type)))
        }
        _ => Err(FiberStateError::InvalidFrame),
    }
}

fn validate_format_state(
    program: &AwbcProgram,
    frame: &FiberFrame,
    state: &FiberFormatState,
) -> Result<(), FiberStateError> {
    if !state.format_context.has_current_data() {
        return Err(FiberStateError::InvalidFrame);
    }
    if state.site.function != frame.function {
        return Err(FiberStateError::InvalidFrame);
    }
    let operand_count = format_operand_count_at_site(program, state.site)?;
    if state.values.len() != operand_count || state.next_operand > operand_count {
        return Err(FiberStateError::InvalidFrame);
    }
    let mut had_failure = false;
    for (ordinal, value) in state.values.iter().enumerate() {
        if ordinal >= state.next_operand {
            if value.is_some() {
                return Err(FiberStateError::InvalidFrame);
            }
            continue;
        }
        if let Some(value) = value.as_ref() {
            let result = format_operand_type_at_site(program, state.site, ordinal)?;
            validate_runtime_value_at(
                program,
                value,
                Some(result),
                format!("format.values[{ordinal}]"),
            )?;
        } else {
            had_failure = true;
        }
    }
    if had_failure != state.first_recoverable.is_some() {
        return Err(FiberStateError::InvalidFrame);
    }
    if let Some((method, result_register, result_type)) =
        format_project_method_at_site(program, state.site)?
    {
        let layout = program
            .frame_layouts
            .get(frame.layout.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        if method.receiver != AwbcTraitReceiverMode::Owned
            || method.receiver_state_slot.is_some()
            || layout
                .slots
                .get(result_register.index())
                .is_none_or(|slot| slot.ty != result_type)
            || (state.next_operand < operand_count
                && frame
                    .registers
                    .get(result_register.index())
                    .is_some_and(|storage| !storage.is_vacant()))
            || (state.first_recoverable.is_some()
                && frame
                    .registers
                    .get(result_register.index())
                    .is_some_and(|storage| !storage.is_vacant()))
        {
            return Err(FiberStateError::InvalidFrame);
        }
    } else if let AwbcInstruction::FormatContent {
        project_option: true,
        ..
    } = instruction_at_site(program, state.site)?
    {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn scope_has_pending_cleanup(scope: &FiberScope) -> bool {
    !scope.cleanups.is_empty()
        || !scope.defers.is_empty()
        || !scope.defer_releasing.is_empty()
        || scope.defer_inflight.is_some()
        || scope.defer_exit.is_some()
        || scope.defer_failure.is_some()
}

fn validate_frame(
    program: &AwbcProgram,
    frame: &FiberFrame,
    path: &str,
) -> Result<(), FiberStateError> {
    let function = program
        .functions
        .get(frame.function.index())
        .ok_or(FiberStateError::UnknownFunction(frame.function.0))?;
    if function.frame_layout != frame.layout {
        return Err(FiberStateError::InvalidFrame);
    }
    match (function.type_context, frame.type_instantiation.as_deref()) {
        (None, None) => {}
        (Some(context), Some(binding))
            if program
                .runtime_types
                .get(context.index())
                .is_some_and(|row| row.semantic_identity() == binding.context())
                && binding.is_valid(program) => {}
        _ => return Err(FiberStateError::InvalidFrame),
    }
    let layout = program
        .frame_layouts
        .get(frame.layout.index())
        .ok_or(FiberStateError::UnknownFrameLayout(frame.layout.0))?;
    if frame.registers.len() != layout.slots.len() {
        return Err(FiberStateError::InvalidFrame);
    }
    for (index, value) in frame.registers.iter().enumerate() {
        let slot = layout
            .slots
            .get(index)
            .ok_or(FiberStateError::InvalidFrame)?;
        validate_place_storage_at(
            program,
            frame.type_instantiation.as_deref(),
            value,
            slot.ty,
            format!("{path}.registers[{index}]"),
            0,
        )?;
    }
    if let Some(format) = &frame.format {
        validate_format_state(program, frame, format)?;
    }
    for attempt in &frame.format_attempts {
        format::validate_format_attempt_state(program, frame, attempt)?;
    }
    for (index, cleanup) in frame.root_cleanups.iter().enumerate() {
        validate_cleanup(program, cleanup, &format!("{path}.root_cleanups[{index}]"))?;
    }
    let mut defer_ids = BTreeSet::new();
    for (index, deferred) in frame.root_defers.iter().enumerate() {
        if !defer_ids.insert(deferred.id) {
            return Err(FiberStateError::InvalidFrame);
        }
        validate_deferred(
            program,
            deferred,
            layout.slots.len(),
            &format!("{path}.root_defers[{index}]"),
        )?;
    }
    for (scope_index, scope) in frame.scopes.iter().enumerate() {
        let definition = layout
            .scopes
            .get(scope.id.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let expected_parent = scope_index
            .checked_sub(1)
            .map(|index| frame.scopes[index].id);
        if scope.depth as usize != scope_index
            || scope_index >= layout.max_scope_depth as usize
            || definition.parent != expected_parent
            || (scope.defer_inflight.is_some() && scope.defer_exit.is_none())
            || (scope.defer_failure.is_some() && scope.defer_exit.is_none())
            || (!scope.defer_releasing.is_empty() && scope.defer_exit.is_none())
        {
            return Err(FiberStateError::InvalidFrame);
        }
        for (cleanup_index, cleanup) in scope.cleanups.iter().enumerate() {
            validate_cleanup(
                program,
                cleanup,
                &format!("{path}.scopes[{scope_index}].cleanups[{cleanup_index}]"),
            )?;
        }
        for (defer_index, deferred) in scope.defers.iter().enumerate() {
            if !defer_ids.insert(deferred.id) {
                return Err(FiberStateError::InvalidFrame);
            }
            validate_deferred(
                program,
                deferred,
                layout.slots.len(),
                &format!("{path}.scopes[{scope_index}].defers[{defer_index}]"),
            )?;
        }
        if let Some(inflight) = scope.defer_inflight
            && !defer_ids.insert(inflight.registration)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        for release in &scope.defer_releasing {
            if !defer_ids.insert(release.registration)
                || release.tokens.is_empty()
                || release.tokens.iter().collect::<BTreeSet<_>>().len() != release.tokens.len()
            {
                return Err(FiberStateError::InvalidFrame);
            }
        }
    }
    Ok(())
}

fn validate_deferred(
    program: &AwbcProgram,
    deferred: &FiberDeferredRegistration,
    register_count: usize,
    path: &str,
) -> Result<(), FiberStateError> {
    let function_id = program
        .defer_sites
        .get(deferred.site.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let function = program
        .functions
        .get(function_id.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let signature = program
        .signatures
        .get(function.signature.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if deferred.capture_registers.len() != deferred.captures.len()
        || signature.params.len() != deferred.captures.len()
        || !signature.result.is_some_and(|result| {
            matches!(
                program
                    .runtime_types
                    .get(result.index())
                    .map(AwbcRuntimeType::shape),
                Some(AwbcRuntimeTypeShape::Unit)
            )
        })
    {
        return Err(FiberStateError::InvalidFrame);
    }
    let mut seen_registers = std::collections::BTreeSet::new();
    for (index, (register, capture)) in deferred
        .capture_registers
        .iter()
        .zip(&deferred.captures)
        .enumerate()
    {
        if register.index() >= register_count
            || !seen_registers.insert(*register) && !capture.ownership().permits_copy()
        {
            return Err(FiberStateError::InvalidFrame);
        }
        validate_runtime_value_at(
            program,
            capture,
            Some(signature.params[index]),
            format!("{path}.captures[{index}]"),
        )?;
    }
    Ok(())
}

fn validate_place_storage_at(
    program: &AwbcProgram,
    instantiation: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    storage: &RuntimePlaceStorage<RuntimeValue>,
    expected: AwbcTypeId,
    path: String,
    depth: usize,
) -> Result<(), FiberStateError> {
    let row = program
        .runtime_types
        .get(expected.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH
        || !storage.matches_program_type(
            crate::program_types::RuntimeProgramTypes::Awbc(program),
            row.semantic_identity(),
            instantiation,
        )
    {
        return Err(FiberStateError::InvalidRuntimeValue {
            path,
            reason: "place does not match the frame's instantiated type and record schema".into(),
        });
    }
    for (ordinal, value) in storage.values().enumerate() {
        validate_runtime_value_at(program, value, None, format!("{path}.values[{ordinal}]"))?;
    }
    Ok(())
}

fn validate_runtime_value_at(
    program: &AwbcProgram,
    value: &RuntimeValue,
    expected: Option<AwbcTypeId>,
    path: String,
) -> Result<(), FiberStateError> {
    if let Some(expected) = expected
        && !runtime_value_matches_type(program, value, expected, 0)
    {
        return Err(FiberStateError::InvalidRuntimeValue {
            path,
            reason: format!(
                "expected {}, received {}",
                runtime_type_label(program, expected),
                runtime_value_type_label(value)
            ),
        });
    }
    validate_nested_runtime_value(program, value, 0).map_err(|error| {
        FiberStateError::InvalidRuntimeValue {
            path,
            reason: error.to_string(),
        }
    })
}

fn validate_nested_runtime_value(
    program: &AwbcProgram,
    value: &RuntimeValue,
    depth: usize,
) -> Result<(), FiberStateError> {
    if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
        return Err(FiberStateError::InvalidRuntimeCallable {
            reason: format!(
                "runtime value nesting exceeds {} levels",
                crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH
            ),
        });
    }
    match value {
        RuntimeValue::Callable(callable) => validate_runtime_callable(program, callable, depth),
        RuntimeValue::Tuple(items) => items
            .iter()
            .try_for_each(|item| validate_nested_runtime_value(program, item, depth + 1)),
        RuntimeValue::Seq(sequence) => validate_nested_runtime_sequence(program, sequence, depth),
        RuntimeValue::Record(fields) => fields
            .iter()
            .try_for_each(|field| validate_nested_runtime_value(program, field.value(), depth + 1)),
        RuntimeValue::NominalRecord(record) => record
            .fields()
            .iter()
            .try_for_each(|field| validate_nested_runtime_value(program, field, depth + 1)),
        RuntimeValue::Opaque(value) => {
            validate_nested_runtime_value(program, value.payload(), depth + 1)
        }
        // Reduction is a typed producer-owned carrier, but AWBC has not yet
        // admitted a runtime type that retains its owner and generic state
        // projection. Reject it at the durable fiber boundary instead of
        // accepting it through Dynamic and losing that authority on restore.
        RuntimeValue::Reduction(_) => {
            Err(FiberStateError::UnsupportedSnapshotValue { kind: "Reduction" })
        }
        RuntimeValue::Agent(value) => {
            if depth.saturating_add(value.structural_nesting_depth())
                > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH
            {
                return Err(FiberStateError::InvalidRuntimeCallable {
                    reason: format!(
                        "runtime value nesting exceeds {} levels",
                        crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH
                    ),
                });
            }
            value
                .nested_runtime_values_with_depth()
                .into_iter()
                .try_for_each(|(offset, value)| {
                    validate_nested_runtime_value(program, value, depth.saturating_add(offset))
                })
        }
        RuntimeValue::Iterator(RuntimeIterator::Values { items, .. }) => items
            .iter()
            .try_for_each(|item| validate_nested_runtime_value(program, item, depth + 1)),
        RuntimeValue::Iterator(RuntimeIterator::Witness { state, .. }) => {
            validate_nested_runtime_value(program, state, depth + 1)
        }
        RuntimeValue::Variant {
            payload: Some(payload),
            ..
        } => validate_nested_runtime_value(program, payload, depth + 1),
        RuntimeValue::Unit
        | RuntimeValue::Bool(_)
        | RuntimeValue::Int(_)
        | RuntimeValue::UInt(_)
        | RuntimeValue::F32(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::String(_)
        | RuntimeValue::Color(_)
        | RuntimeValue::NeedHandle(_)
        | RuntimeValue::Char(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::Progress(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(RuntimeIterator::Range(_))
        | RuntimeValue::EntityRef(_)
        | RuntimeValue::Variant { payload: None, .. } => Ok(()),
    }
}

fn validate_runtime_callable(
    program: &AwbcProgram,
    callable: &RuntimeCallableValue,
    depth: usize,
) -> Result<(), FiberStateError> {
    if !matches!(
        callable.owner(),
        RuntimeProgramOwner::Awbc(owner) if std::ptr::eq(owner.as_ref(), program)
    ) {
        return Err(FiberStateError::InvalidRuntimeCallable {
            reason: "callable is leased to a different AWBC program".to_owned(),
        });
    }
    callable
        .validate_retained()
        .map_err(|error| FiberStateError::InvalidRuntimeCallable {
            reason: error.to_string(),
        })?;
    let definition = program
        .callable_states
        .get(callable.state().index())
        .ok_or_else(|| FiberStateError::InvalidRuntimeCallable {
            reason: format!("callable state {} is absent", callable.state()),
        })?;
    for (position, (retained, value)) in definition
        .retained
        .iter()
        .zip(callable.retained())
        .enumerate()
    {
        if !runtime_value_matches_type(program, value, retained.ty, depth + 1) {
            return Err(FiberStateError::InvalidRuntimeCallable {
                reason: format!(
                    "callable retained value {position} has type {} instead of {}",
                    runtime_value_type_label(value),
                    runtime_type_label(program, retained.ty)
                ),
            });
        }
        validate_nested_runtime_value(program, value, depth + 1)?;
    }
    Ok(())
}

fn validate_nested_runtime_sequence(
    program: &AwbcProgram,
    sequence: &RuntimeSeq,
    depth: usize,
) -> Result<(), FiberStateError> {
    match sequence {
        RuntimeSeq::Values(items) => items
            .iter()
            .try_for_each(|item| validate_nested_runtime_value(program, item, depth + 1)),
        RuntimeSeq::TupleColumns(columns) => columns
            .columns()
            .iter()
            .try_for_each(|column| validate_nested_runtime_sequence(program, column, depth + 1)),
        RuntimeSeq::RecordColumns(records) => records.fields().iter().try_for_each(|field| {
            validate_nested_runtime_sequence(program, field.values(), depth + 1)
        }),
        RuntimeSeq::Dense(_) => Ok(()),
    }
}

/// Preflights one zero-argument callback without moving or copying its capture
/// values. The resulting proof is consumed only after every callback in the
/// enclosing Product event batch has passed validation.
pub(crate) fn validate_runtime_callable_activation(
    program: &AwbcProgram,
    callable: &RuntimeCallableValue,
) -> Result<PreparedCallableCallbackActivation, FiberStateError> {
    validate_runtime_callable(program, callable, 0)?;
    let proof = callable.inspect_zero_arg_invocation().map_err(|error| {
        FiberStateError::InvalidRuntimeCallable {
            reason: error.to_string(),
        }
    })?;
    let crate::value::RuntimeCallableBodyReference::Awbc(function) = proof.body() else {
        return Err(FiberStateError::InvalidRuntimeCallable {
            reason: "callback callable must invoke one AWBC function body".to_owned(),
        });
    };
    let definition = program
        .callable_states
        .get(callable.state().index())
        .ok_or_else(|| FiberStateError::InvalidRuntimeCallable {
            reason: format!("callable state {} is absent", callable.state()),
        })?;
    if !definition.parameters.is_empty()
        || !matches!(definition.attached, RuntimeCallableAttachedContract::None)
    {
        return Err(FiberStateError::InvalidRuntimeCallable {
            reason: "dialogue callback must have a zero-argument, unattached callable state"
                .to_owned(),
        });
    }
    let RuntimeCallableTransition::Invoke {
        function: transition_function,
        captures,
        arguments,
    } = &definition.transition
    else {
        return Err(FiberStateError::InvalidRuntimeCallable {
            reason: "dialogue callback callable state does not invoke".to_owned(),
        });
    };
    if *transition_function != function || !arguments.is_empty() {
        return Err(FiberStateError::InvalidRuntimeCallable {
            reason: "dialogue callback invocation projection is not zero-argument".to_owned(),
        });
    }
    let function_record = program
        .functions
        .get(function.index())
        .ok_or(FiberStateError::UnknownFunction(function.0))?;
    if function_record.kind != AwbcFunctionKind::Ordinary {
        return Err(FiberStateError::InvalidRuntimeCallable {
            reason: "dialogue callback is not an ordinary executable callable body".to_owned(),
        });
    }
    let signature = program
        .signatures
        .get(function_record.signature.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if signature.result.is_some() || signature.params.len() != captures.len() {
        return Err(FiberStateError::InvalidRuntimeCallable {
            reason: "dialogue callback must return Unit and accept exactly its captures".to_owned(),
        });
    }
    let mut capture_values = Vec::with_capacity(captures.len());
    for (position, (source, expected)) in captures.iter().zip(&signature.params).enumerate() {
        let RuntimeCallableInputSource::Retained { position: retained } = source else {
            return Err(FiberStateError::InvalidRuntimeCallable {
                reason: "dialogue callback capture projection references a non-retained input"
                    .to_owned(),
            });
        };
        let value = callable.retained().get(*retained as usize).ok_or_else(|| {
            FiberStateError::InvalidRuntimeCallable {
                reason: "dialogue callback capture projection is out of range".to_owned(),
            }
        })?;
        if !super::vm::runtime_value_view_matches_type(program, value.view(), *expected, 0) {
            return Err(FiberStateError::InvalidRuntimeCallable {
                reason: format!(
                    "dialogue callback capture {position} does not match its function parameter"
                ),
            });
        }
        capture_values.push(value);
    }
    let input_layout = validate_function_argument_value_refs(program, function, &capture_values)?;
    Ok(PreparedCallableCallbackActivation {
        function,
        input_layout,
        proof,
    })
}

/// Borrowed preflight for any positional function activation. The returned
/// slot proof may be reused only with the exact owner packet that the caller
/// kept untouched while validation ran.
pub(crate) fn validate_function_argument_values(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    values: &[RuntimeValue],
) -> Result<PreparedFunctionInputBinding, FiberStateError> {
    let references = values.iter().collect::<Vec<_>>();
    validate_function_argument_value_refs(program, function, &references)
}

pub(crate) fn validate_function_argument_value_refs(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    values: &[&RuntimeValue],
) -> Result<PreparedFunctionInputBinding, FiberStateError> {
    let function_record = program
        .functions
        .get(function.index())
        .ok_or(FiberStateError::UnknownFunction(function.0))?;
    let signature = program
        .signatures
        .get(function_record.signature.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let layout = program
        .frame_layouts
        .get(function_record.frame_layout.index())
        .ok_or(FiberStateError::UnknownFrameLayout(
            function_record.frame_layout.0,
        ))?;
    let parameter_registers = layout
        .slots
        .iter()
        .enumerate()
        .filter_map(|(register, slot)| {
            (slot.role == AwbcFrameSlotRole::Parameter)
                .then(|| u32::try_from(register).ok().map(AwbcRegisterId))
                .flatten()
        })
        .collect::<Vec<_>>();
    if parameter_registers.len() != signature.params.len() || values.len() != signature.params.len()
    {
        return Err(FiberStateError::ArgumentCount {
            expected: parameter_registers.len(),
            actual: values.len(),
        });
    }
    let type_instantiation =
        FiberFrame::instantiate_context(program, function_record.type_context, values)?;
    for (position, ((register, slot), value)) in parameter_registers
        .iter()
        .map(|register| (*register, &layout.slots[register.index()]))
        .zip(values)
        .enumerate()
    {
        let expected = signature.params[position];
        if slot.ty != expected
            || !FiberFrame::value_matches_instantiation(
                program,
                type_instantiation.as_ref(),
                value,
                expected,
            )
        {
            return Err(FiberStateError::ArgumentType {
                name: slot
                    .name
                    .and_then(|name| program.strings.get(name.index()).cloned())
                    .unwrap_or_else(|| format!("${position}")),
                expected: runtime_type_label(program, expected),
                actual: runtime_value_type_label(value),
            });
        }
        if register.index() >= layout.slots.len() {
            return Err(FiberStateError::InvalidFrame);
        }
    }
    super::vm::validate_function_input_ownership_values(program, function, values).map_err(
        |error| FiberStateError::InvalidFunctionInputOwnership {
            reason: error.to_string(),
        },
    )?;
    Ok(PreparedFunctionInputBinding {
        function,
        type_instantiation,
        parameter_registers: parameter_registers.into_boxed_slice(),
    })
}

fn validate_cleanup(
    program: &AwbcProgram,
    cleanup: &FiberScopeCleanup,
    path: &str,
) -> Result<(), FiberStateError> {
    if cleanup.key.is_empty() {
        return Err(FiberStateError::InvalidFrame);
    }
    let effect = program
        .effect_plans
        .get(cleanup.effect.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let signature = program
        .signatures
        .get(effect.signature.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if cleanup.args.len() != signature.params.len() {
        return Err(FiberStateError::InvalidRuntimeValue {
            path: format!("{path}.args"),
            reason: format!(
                "cleanup effect expects {} arguments, snapshot stores {}",
                signature.params.len(),
                cleanup.args.len()
            ),
        });
    }
    for (index, (value, expected)) in cleanup.args.iter().zip(&signature.params).enumerate() {
        validate_runtime_value_at(
            program,
            value,
            Some(*expected),
            format!("{path}.args[{index}]"),
        )?;
    }
    Ok(())
}

fn validate_return_point(
    program: &AwbcProgram,
    caller: &FiberFrame,
    returning_function: AwbcFunctionId,
    return_to: &FiberReturnPoint,
) -> Result<(), FiberStateError> {
    validate_cursor(program, return_to.cursor)?;
    if return_to.cursor.function != caller.function {
        return Err(FiberStateError::InvalidFrame);
    }
    if let Some(destination) = return_to.destination
        && destination.index() >= caller.registers.len()
    {
        return Err(FiberStateError::RegisterOutOfBounds {
            register: destination.0,
            layout: caller.layout.0,
        });
    }
    validate_return_continuation(program, caller, returning_function, return_to)?;
    Ok(())
}

fn validate_return_continuation(
    program: &AwbcProgram,
    caller: &FiberFrame,
    returning_function: AwbcFunctionId,
    return_to: &FiberReturnPoint,
) -> Result<(), FiberStateError> {
    let (site, default_stage) = match &return_to.continuation {
        FiberReturnContinuation::Ordinary => return Ok(()),
        FiberReturnContinuation::FormatOperand { site, ordinal } => {
            return validate_format_operand_return(
                program,
                caller,
                returning_function,
                return_to,
                *site,
                *ordinal,
            );
        }
        FiberReturnContinuation::FormatDisplay { site } => {
            return validate_format_display_return(
                program,
                caller,
                returning_function,
                return_to,
                *site,
            );
        }
        FiberReturnContinuation::InstructionCall { site } => {
            return validate_instruction_call_return(
                program,
                caller,
                returning_function,
                return_to,
                *site,
            );
        }
        FiberReturnContinuation::ContextCallbackDefault {
            site,
            pending,
            callable_pending,
        } => {
            return validate_context_callback_return(
                program,
                caller,
                returning_function,
                return_to,
                *site,
                pending,
                true,
                callable_pending.state(),
            );
        }
        FiberReturnContinuation::ContextCallbackInvoke {
            site,
            pending,
            callable_state,
        } => {
            return validate_context_callback_return(
                program,
                caller,
                returning_function,
                return_to,
                *site,
                pending,
                false,
                *callable_state,
            );
        }
        FiberReturnContinuation::ProjectCallDefault { site, pending } => {
            validate_project_call_stage_values(
                program,
                caller,
                returning_function,
                return_to,
                *site,
                Some(pending),
                true,
            )?;
            (*site, true)
        }
        FiberReturnContinuation::ProjectCallTarget { site } => {
            validate_project_call_stage_values(
                program,
                caller,
                returning_function,
                return_to,
                *site,
                None,
                false,
            )?;
            (*site, false)
        }
        FiberReturnContinuation::ApplyGroupDefault {
            pending,
            destination,
        } => {
            if destination.index() >= caller.registers.len() || return_to.destination.is_some() {
                return Err(FiberStateError::InvalidFrame);
            }
            if !pending_default_matches(program, pending.state(), returning_function) {
                return Err(FiberStateError::InvalidFrame);
            }
            return Ok(());
        }
    };
    if site.caller_function != caller.function {
        return Err(FiberStateError::InvalidFrame);
    }
    let block = program
        .blocks
        .get(site.block.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if block.owner != caller.function {
        return Err(FiberStateError::InvalidFrame);
    }
    let super::schema::AwbcTerminator::ProjectCall { call } = &block.terminator else {
        return Err(FiberStateError::InvalidFrame);
    };
    let resume = program
        .resume_points
        .get(call.resume.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if resume.function != caller.function
        || resume.frame_layout != caller.layout
        || return_to.cursor.function != caller.function
        || return_to.cursor.block != resume.block
        || return_to.cursor.instruction_offset != 0
    {
        return Err(FiberStateError::InvalidFrame);
    }
    let Some(state) = program.callable_states.get(call.state.index()) else {
        return Err(FiberStateError::InvalidFrame);
    };
    if program.runtime_types.get(state.result.index()).is_none()
        || program.patterns.get(call.result_pattern.index()).is_none()
    {
        return Err(FiberStateError::InvalidFrame);
    }
    match (default_stage, &state.attached, &state.transition) {
        (true, crate::plan::RuntimeCallableAttachedContract::Defaulted { default, .. }, _) => {
            let Some(attached) = call.attached.as_ref() else {
                return Err(FiberStateError::InvalidFrame);
            };
            if attached.presence != super::schema::AwbcProjectCallAttachedPresence::DefaultedOmitted
            {
                return Err(FiberStateError::InvalidFrame);
            }
            let crate::plan::RuntimeCallableDefault::Body { function, .. } = default else {
                return Err(FiberStateError::InvalidFrame);
            };
            if returning_function != *function || program.functions.get(function.index()).is_none()
            {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        (false, _, crate::plan::RuntimeCallableTransition::Invoke { function, .. })
            if returning_function == *function
                && program.functions.get(function.index()).is_some() => {}
        _ => return Err(FiberStateError::InvalidFrame),
    }
    Ok(())
}

fn validate_format_operand_return(
    program: &AwbcProgram,
    caller: &FiberFrame,
    returning_function: AwbcFunctionId,
    return_to: &FiberReturnPoint,
    site: FiberCursor,
    ordinal: usize,
) -> Result<(), FiberStateError> {
    if return_to.cursor != site || return_to.destination.is_some() {
        return Err(FiberStateError::InvalidFrame);
    }
    let state = caller
        .format
        .as_ref()
        .ok_or(FiberStateError::InvalidFrame)?;
    validate_format_state(program, caller, state)?;
    if state.site != site || state.next_operand != ordinal {
        return Err(FiberStateError::InvalidFrame);
    }
    let operand = format_operands_at_site(program, site)?
        .get(ordinal)
        .ok_or(FiberStateError::InvalidFrame)?;
    let signature = program
        .functions
        .get(operand.function.index())
        .and_then(|function| program.signatures.get(function.signature.index()))
        .ok_or(FiberStateError::InvalidFrame)?;
    let layout = program
        .frame_layouts
        .get(caller.layout.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if returning_function != operand.function
        || signature.result.is_none()
        || operand.captures.len() != signature.params.len()
        || operand
            .captures
            .iter()
            .zip(&signature.params)
            .any(|(capture, ty)| {
                layout
                    .slots
                    .get(capture.index())
                    .is_none_or(|slot| slot.ty != *ty)
            })
    {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn validate_format_display_return(
    program: &AwbcProgram,
    caller: &FiberFrame,
    returning_function: AwbcFunctionId,
    return_to: &FiberReturnPoint,
    site: FiberCursor,
) -> Result<(), FiberStateError> {
    if return_to.cursor != site || site.function != caller.function {
        return Err(FiberStateError::InvalidFrame);
    }
    let state = caller
        .format
        .as_ref()
        .ok_or(FiberStateError::InvalidFrame)?;
    validate_format_state(program, caller, state)?;
    let operand_count = format_operand_count_at_site(program, site)?;
    if state.site != site
        || state.next_operand != operand_count
        || state.first_recoverable.is_some()
    {
        return Err(FiberStateError::InvalidFrame);
    }
    let AwbcInstruction::FormatContent {
        project_method: Some(method_id),
        project_result: Some(result_register),
        ..
    } = instruction_at_site(program, site)?
    else {
        return Err(FiberStateError::InvalidFrame);
    };
    let method = program
        .trait_methods
        .get(method_id.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let function = program
        .functions
        .get(method.function.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let signature = program
        .signatures
        .get(function.signature.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let destination_type = program
        .frame_layouts
        .get(caller.layout.index())
        .and_then(|layout| layout.slots.get(result_register.index()))
        .map(|slot| slot.ty)
        .ok_or(FiberStateError::InvalidFrame)?;
    if method.receiver != AwbcTraitReceiverMode::Owned
        || method.receiver_state_slot.is_some()
        || returning_function != method.function
        || return_to.destination != Some(*result_register)
        || signature.params.len() != 2
        || signature.result != Some(destination_type)
    {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn validate_instruction_call_return(
    program: &AwbcProgram,
    caller: &FiberFrame,
    returning_function: AwbcFunctionId,
    return_to: &FiberReturnPoint,
    site: FiberCursor,
) -> Result<(), FiberStateError> {
    let next_offset = site
        .instruction_offset
        .checked_add(1)
        .ok_or(FiberStateError::InvalidFrame)?;
    if site.function != caller.function
        || return_to.cursor
            != (FiberCursor {
                instruction_offset: next_offset,
                ..site
            })
    {
        return Err(FiberStateError::InvalidFrame);
    }
    let (destination, function, receiver_out) = match instruction_at_site(program, site)? {
        AwbcInstruction::CallPureHelper { dst, helper, .. } => {
            let function = program
                .pure_helpers
                .get(helper.index())
                .ok_or(FiberStateError::InvalidFrame)?
                .function;
            (*dst, function, None)
        }
        AwbcInstruction::CallTraitMethod {
            dst,
            method,
            receiver_out,
            ..
        } => {
            let method = program
                .trait_methods
                .get(method.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            if receiver_out.is_some() != (method.receiver == AwbcTraitReceiverMode::MutRef) {
                return Err(FiberStateError::InvalidFrame);
            }
            (*dst, method.function, *receiver_out)
        }
        _ => return Err(FiberStateError::InvalidFrame),
    };
    if return_to.destination != Some(destination)
        || returning_function != function
        || receiver_out.is_some_and(|register| register.index() >= caller.registers.len())
    {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn instruction_call_receiver_update(
    program: &AwbcProgram,
    caller: &FiberFrame,
    callee: &FiberFrame,
    continuation: &FiberReturnContinuation,
) -> Result<Option<(AwbcRegisterId, RuntimeValue)>, FiberStateError> {
    let FiberReturnContinuation::InstructionCall { site } = continuation else {
        return Ok(None);
    };
    let AwbcInstruction::CallTraitMethod {
        method,
        receiver_out: Some(destination),
        ..
    } = instruction_at_site(program, *site)?
    else {
        return Ok(None);
    };
    let method = program
        .trait_methods
        .get(method.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let slot = method
        .receiver_state_slot
        .ok_or(FiberStateError::InvalidFrame)?;
    let value = callee.register(slot)?.clone();
    let expected = program
        .frame_layouts
        .get(caller.layout.index())
        .and_then(|layout| layout.slots.get(destination.index()))
        .map(|slot| slot.ty)
        .ok_or(FiberStateError::InvalidFrame)?;
    if !caller.value_matches_type(program, &value, expected) {
        return Err(FiberStateError::ReturnValueMismatch);
    }
    Ok(Some((*destination, value)))
}

#[allow(
    clippy::too_many_arguments,
    reason = "snapshot validation keeps the exact callback stage and return site visible"
)]
fn validate_context_callback_return(
    program: &AwbcProgram,
    caller: &FiberFrame,
    returning_function: AwbcFunctionId,
    return_to: &FiberReturnPoint,
    site: FiberCursor,
    pending: &RuntimeArcErrorContextPending,
    default_stage: bool,
    callable_state: crate::runtime_id::RuntimeCallableStateId,
) -> Result<(), FiberStateError> {
    if site.function != caller.function
        || return_to.cursor != site
        || return_to.destination.is_some()
    {
        return Err(FiberStateError::InvalidFrame);
    }
    let AwbcInstruction::CallIntrinsic {
        dst: Some(_),
        intrinsic,
        args,
    } = instruction_at_site(program, site)?
    else {
        return Err(FiberStateError::InvalidFrame);
    };
    let [_, _] = args.as_slice() else {
        return Err(FiberStateError::InvalidFrame);
    };
    let kind = match program
        .intrinsics
        .get(intrinsic.index())
        .and_then(|record| record.identity.as_intrinsic())
    {
        Some(crate::value::RuntimeIntrinsic::StdResultWithContext) => {
            RuntimeArcErrorContextKind::Result
        }
        Some(crate::value::RuntimeIntrinsic::StdOptionWithContext) => {
            RuntimeArcErrorContextKind::Option
        }
        _ => return Err(FiberStateError::InvalidFrame),
    };
    if !matches!(
        (kind, pending),
        (
            RuntimeArcErrorContextKind::Result,
            RuntimeArcErrorContextPending::ResultErr(_)
        ) | (
            RuntimeArcErrorContextKind::Option,
            RuntimeArcErrorContextPending::OptionNone
        )
    ) {
        return Err(FiberStateError::InvalidFrame);
    }
    let intrinsic = program
        .intrinsics
        .get(intrinsic.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let signature = program
        .signatures
        .get(intrinsic.signature.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let Some(callback_type) = signature.params.get(1).copied() else {
        return Err(FiberStateError::InvalidFrame);
    };
    let state = program
        .callable_states
        .get(callable_state.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if state.function_type != callback_type
        || (default_stage && !pending_default_matches(program, callable_state, returning_function))
        || (!default_stage
            && !matches!(
                state.transition,
                crate::plan::RuntimeCallableTransition::Invoke { function, .. }
                    if function == returning_function
            ))
    {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn validate_project_call_stage_values(
    program: &AwbcProgram,
    caller: &FiberFrame,
    returning_function: AwbcFunctionId,
    return_to: &FiberReturnPoint,
    site: AwbcProjectCallSite,
    pending: Option<&RuntimeCallablePendingGroup>,
    default_stage: bool,
) -> Result<(), FiberStateError> {
    let block = program
        .blocks
        .get(site.block.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let super::schema::AwbcTerminator::ProjectCall { call } = &block.terminator else {
        return Err(FiberStateError::InvalidFrame);
    };
    let state = program
        .callable_states
        .get(call.state.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if return_to.destination.is_some()
        || state.result.index() >= program.runtime_types.len()
        || call.result_pattern.index() >= program.patterns.len()
    {
        return Err(FiberStateError::InvalidFrame);
    }
    if default_stage {
        if pending.is_none_or(|pending| {
            pending.state() != call.state
                || !pending_default_matches(program, pending.state(), returning_function)
        }) {
            return Err(FiberStateError::InvalidFrame);
        }
    } else if pending.is_some()
        || !matches!(
            state.transition,
            crate::plan::RuntimeCallableTransition::Invoke { function, .. }
                if function == returning_function
        )
    {
        return Err(FiberStateError::InvalidFrame);
    }
    if site.caller_function != caller.function {
        return Err(FiberStateError::InvalidFrame);
    }
    if caller
        .registers
        .get(call.callee.index())
        .is_none_or(|storage| !storage.is_vacant())
        || call.operands.iter().any(|operand| {
            caller
                .registers
                .get(operand.value.index())
                .is_none_or(|storage| !storage.is_vacant())
        })
    {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn pending_default_matches(
    program: &AwbcProgram,
    state_id: crate::runtime_id::RuntimeCallableStateId,
    returning_function: AwbcFunctionId,
) -> bool {
    let Some(state) = program.callable_states.get(state_id.index()) else {
        return false;
    };
    matches!(
        &state.attached,
        crate::plan::RuntimeCallableAttachedContract::Defaulted {
            default: crate::plan::RuntimeCallableDefault::Body { function, .. },
            ..
        } if *function == returning_function
    )
}

fn validate_resume_point<'a>(
    program: &'a AwbcProgram,
    frame: &FiberFrame,
    resume: AwbcResumePointId,
) -> Result<&'a super::schema::AwbcResumePoint, FiberStateError> {
    let point = program
        .resume_points
        .get(resume.index())
        .ok_or(FiberStateError::UnknownResumePoint(resume.0))?;
    if point.function != frame.function {
        return Err(FiberStateError::ResumeFunctionMismatch {
            resume: resume.0,
            actual: point.function.0,
            expected: frame.function.0,
        });
    }
    if point.frame_layout != frame.layout {
        return Err(FiberStateError::ResumeLayoutMismatch {
            resume: resume.0,
            actual: point.frame_layout.0,
            expected: frame.layout.0,
        });
    }
    Ok(point)
}

fn validate_suspension(
    program: &AwbcProgram,
    state: &FiberState,
    suspension: &FiberSuspension,
) -> Result<(), FiberStateError> {
    let frame = state.active_frame()?;
    match suspension.resume {
        FiberResumeTarget::Declared(resume) => {
            validate_resume_point(program, frame, resume)?;
        }
        FiberResumeTarget::Exact(cursor) => {
            if suspension.reason != FiberSuspensionReason::BudgetYield
                || cursor != state.cursor
                || cursor.function != frame.function
            {
                return Err(FiberStateError::InvalidFrame);
            }
            validate_cursor(program, cursor)?;
        }
    }
    match &suspension.reason {
        FiberSuspensionReason::Dialogue {
            target,
            target_type,
            content,
            values,
            effects,
            line_task_captures,
            result,
        } => {
            if target.as_ref().is_some_and(|target| {
                !dialogue_target_opaque_matches_program(program, *target_type, target)
            }) || (target.is_none()
                && !dialogue_target_type_matches_program(program, *target_type))
            {
                return Err(FiberStateError::InvalidFrame);
            }
            let Some(content) = program.content_units.get(content.index()) else {
                return Err(FiberStateError::InvalidFrame);
            };
            let Some(template) = program
                .content_templates
                .iter()
                .find(|template| template.id == content.template)
            else {
                return Err(FiberStateError::InvalidFrame);
            };
            if effects.len() != template.effects.len() {
                return Err(FiberStateError::InvalidFrame);
            }
            for (index, effect) in effects.iter().enumerate() {
                let Some(declared) = template.effects.get(index) else {
                    return Err(FiberStateError::InvalidFrame);
                };
                let Some(expected_site) =
                    crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                else {
                    return Err(FiberStateError::InvalidFrame);
                };
                if effect.site != expected_site
                    || effect.site != declared.site
                    || program.callable_states.get(effect.state.index()).is_none()
                    || effect.captures.len() != declared.capture_types.len()
                {
                    return Err(FiberStateError::InvalidFrame);
                }
                if effect
                    .captures
                    .iter()
                    .zip(&declared.capture_types)
                    .any(|(value, expected)| {
                        !runtime_value_matches_type(program, value, *expected, 0)
                    })
                {
                    return Err(FiberStateError::InvalidFrame);
                }
            }
            if let Some(group) = content
                .line_task_group
                .and_then(|group| program.line_task_groups.get(group.index()))
            {
                if group.captures.len() != line_task_captures.len() {
                    return Err(FiberStateError::InvalidFrame);
                }
            } else if !line_task_captures.is_empty() {
                return Err(FiberStateError::InvalidFrame);
            }
            for (index, binding) in values.iter().enumerate() {
                if crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index)
                    != Some(binding.slot)
                {
                    return Err(FiberStateError::InvalidFrame);
                }
            }
            if program.runtime_types.get(result.ty.index()).is_none() {
                return Err(FiberStateError::InvalidFrame);
            }
            if result.destination.index() >= frame.registers.len()
                || program.patterns.get(result.pattern.index()).is_none()
            {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        FiberSuspensionReason::Choice {
            choice,
            destination,
        } => {
            if program.choices.get(choice.index()).is_none() {
                return Err(FiberStateError::InvalidFrame);
            }
            if destination.index() >= frame.registers.len() {
                return Err(FiberStateError::RegisterOutOfBounds {
                    register: destination.0,
                    layout: frame.layout.0,
                });
            }
        }
        FiberSuspensionReason::Await {
            target,
            binding,
            observer,
        } => {
            validate_await_suspension(program, frame, target, *binding)?;
            if observer.is_some_and(|observer| {
                observer.destination.index() >= frame.registers.len()
                    || program.resume_points.get(observer.resume.index()).is_none()
            }) {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        FiberSuspensionReason::AwaitMany(await_many) => {
            validate_await_many_suspension(program, state, await_many)?;
        }
        FiberSuspensionReason::HostCall {
            call,
            args,
            destination,
        } => {
            validate_host_call_suspension(program, frame, *call, args, *destination)?;
        }
        FiberSuspensionReason::BudgetYield => {}
    }
    Ok(())
}

pub(crate) fn dialogue_target_matches_program(
    program: &AwbcProgram,
    target_type: AwbcTypeId,
    value: &RuntimeValue,
) -> bool {
    let RuntimeValue::Opaque(target) = value else {
        return false;
    };
    dialogue_target_opaque_matches_program(program, target_type, target)
}

pub(crate) fn dialogue_target_opaque_matches_program(
    program: &AwbcProgram,
    target_type: AwbcTypeId,
    target: &crate::value::RuntimeOpaqueValue,
) -> bool {
    dialogue_target_type_matches_program(program, target_type)
        && program
            .opaque_owner(target_type)
            .ok()
            .flatten()
            .is_some_and(|owner| owner.accepts_opaque_value(target))
        && target.producer() == &crate::value::RuntimeCharacterDialogueProducerId::get()
}

pub(crate) fn dialogue_target_type_matches_program(
    program: &AwbcProgram,
    target_type: AwbcTypeId,
) -> bool {
    let Some(AwbcRuntimeTypeShape::Opaque {
        arguments,
        value_class: crate::value::RuntimeOpaqueValueClass::Plain,
        persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
        ..
    }) = program
        .runtime_types
        .get(target_type.index())
        .map(|runtime_type| runtime_type.shape())
    else {
        return false;
    };
    arguments.is_empty()
        && program
            .opaque_owner(target_type)
            .ok()
            .flatten()
            .is_some_and(|owner| {
                owner.producer() == &crate::value::RuntimeCharacterDialogueProducerId::get()
            })
}

fn validate_await_suspension(
    program: &AwbcProgram,
    frame: &FiberFrame,
    target: &FiberAwaitTarget,
    binding: Option<AwbcPatternId>,
) -> Result<(), FiberStateError> {
    if binding.is_some_and(|binding| program.patterns.get(binding.index()).is_none()) {
        return Err(FiberStateError::InvalidFrame);
    }
    match target {
        FiberAwaitTarget::Need {
            need,
            item_type,
            handle,
        } => {
            let item_type_exists = program.runtime_types.get(item_type.index()).is_some();
            let layout_slot = program
                .frame_layouts
                .get(frame.layout.index())
                .and_then(|layout| layout.slots.get(handle.index()));
            let register_value = frame
                .registers
                .get(handle.index())
                .and_then(RuntimePlaceStorage::as_ref);
            let matches_source = layout_slot.is_some_and(|slot| {
                matches!(
                    program.runtime_types.get(slot.ty.index()).map(AwbcRuntimeType::shape),
                    Some(AwbcRuntimeTypeShape::Need(source_item)) if *source_item == *item_type
                )
            }) && register_value.is_none();
            let payload_matches = program
                .runtime_types
                .get(item_type.index())
                .is_some_and(|ty| {
                    ty.semantic_identity() == need.outcome().payload_semantic_identity()
                });
            if !payload_matches || !item_type_exists || !matches_source {
                return Err(FiberStateError::InvalidRuntimeValue {
                    path: "suspension.await.target".to_owned(),
                    reason: "Need identity or selected item type disagrees with its consumed handle register".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn validate_await_many_suspension(
    program: &AwbcProgram,
    _fiber: &FiberState,
    state: &FiberAwaitManyState,
) -> Result<(), FiberStateError> {
    use crate::awbc::schema::AwbcTaskPlanKind;
    use crate::value::{RuntimeTupleView, RuntimeValueView};
    let plan = program
        .task_plans
        .get(state.plan.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let AwbcTaskPlanKind::AwaitMany {
        child,
        limit,
        captures,
        request_function,
        ..
    } = &plan.kind
    else {
        return Err(FiberStateError::InvalidFrame);
    };
    let child_plan = program
        .task_plans
        .get(child.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let request = program
        .functions
        .get(request_function.index())
        .and_then(|function| program.signatures.get(function.signature.index()))
        .ok_or(FiberStateError::InvalidFrame)?;
    if *limit == 0
        || u32::try_from(state.items.len()).is_err()
        || state.captured.len() != captures.len()
        || request.params.len() != state.captured.len()
        || state.results.len() != state.items.len()
        || state.next_index as usize > state.items.len()
        || state.in_flight.len() > *limit as usize
        || state
            .binding
            .is_some_and(|binding| program.patterns.get(binding.index()).is_none())
    {
        return Err(FiberStateError::InvalidFrame);
    }
    for (index, (value, ty)) in state.captured.iter().zip(&request.params).enumerate() {
        validate_runtime_value_at(
            program,
            value,
            Some(*ty),
            format!("suspension.await_many.captured[{index}]"),
        )?;
        if !value.ownership().permits_copy() {
            return Err(FiberStateError::InvalidFrame);
        }
    }
    let mut pending = BTreeSet::new();
    for child in &state.in_flight {
        if child.index >= state.next_index || !pending.insert(child.index as usize) {
            return Err(FiberStateError::InvalidFrame);
        }
        let item = state
            .items
            .get(child.index as usize)
            .ok_or(FiberStateError::InvalidFrame)?;
        let index = RuntimeValue::u32(child.index);
        let fields = [
            RuntimeValueView::Tuple(RuntimeTupleView::Values(&state.captured)),
            index.view(),
            item.view(),
        ];
        let expected = child_plan
            .instantiate_template(
                program,
                child.handle.correlation().generation,
                RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
                child.handle.spec().request.clone(),
                16 * 1024 * 1024,
            )
            .map_err(|_| FiberStateError::InvalidFrame)?;
        if !expected.same_join_contract(child.handle.spec())
            || expected
                .correlation(child.handle.correlation().launch_ordinal)
                .map_err(|_| FiberStateError::InvalidFrame)?
                != child.handle.correlation()
        {
            return Err(FiberStateError::InvalidFrame);
        }
    }
    if let Some(base) = &state.base {
        let fields = [
            RuntimeValueView::Tuple(RuntimeTupleView::Values(&state.captured)),
            RuntimeValueView::Tuple(RuntimeTupleView::Values(&state.items)),
        ];
        let expected = plan
            .instantiate_template(
                program,
                base.correlation().generation,
                RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
                base.spec().request.clone(),
                16 * 1024 * 1024,
            )
            .map_err(|_| FiberStateError::InvalidFrame)?;
        if !expected.same_join_contract(base.spec())
            || expected
                .correlation(base.correlation().launch_ordinal)
                .map_err(|_| FiberStateError::InvalidFrame)?
                != base.correlation()
            || state
                .in_flight
                .iter()
                .any(|child| child.handle.correlation().generation != base.correlation().generation)
            || !(0..state.items.len()).all(|index| {
                if index < state.next_index as usize {
                    state.results[index].is_some() != pending.contains(&index)
                } else {
                    state.results[index].is_none() && !pending.contains(&index)
                }
            })
        {
            return Err(FiberStateError::InvalidFrame);
        }
    } else if state.next_index != 0
        || !state.in_flight.is_empty()
        || state.results.iter().any(Option::is_some)
    {
        return Err(FiberStateError::InvalidFrame);
    }
    let AwbcTaskPlanKind::Template {
        request_function, ..
    } = child_plan.kind
    else {
        return Err(FiberStateError::InvalidFrame);
    };
    let child_inputs = program
        .functions
        .get(request_function.index())
        .and_then(|function| program.signatures.get(function.signature.index()))
        .ok_or(FiberStateError::InvalidFrame)?;
    let item_type = child_inputs
        .params
        .last()
        .copied()
        .ok_or(FiberStateError::InvalidFrame)?;
    if child_inputs.params.len() != state.captured.len() + 1 {
        return Err(FiberStateError::InvalidFrame);
    }
    for (index, item) in state.items.iter().enumerate() {
        validate_runtime_value_at(
            program,
            item,
            Some(item_type),
            format!("suspension.await_many.items[{index}]"),
        )?;
        if !item.ownership().permits_copy() {
            return Err(FiberStateError::InvalidFrame);
        }
    }
    for (index, result) in state.results.iter().enumerate() {
        if let Some(result) = result {
            validate_runtime_value_at(
                program,
                result,
                Some(child_plan.payload_type),
                format!("suspension.await_many.results[{index}]"),
            )?;
        }
    }
    Ok(())
}
fn validate_host_call_suspension(
    program: &AwbcProgram,
    frame: &FiberFrame,
    call: AwbcHostCallId,
    args: &[RuntimeValue],
    destination: Option<AwbcRegisterId>,
) -> Result<(), FiberStateError> {
    let call = program
        .host_calls
        .get(call.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let signature = program
        .signatures
        .get(call.signature.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if args.len() != signature.params.len() {
        return Err(FiberStateError::InvalidRuntimeValue {
            path: "suspension.host_call.args".to_owned(),
            reason: format!(
                "host call expects {} arguments, snapshot stores {}",
                signature.params.len(),
                args.len()
            ),
        });
    }
    for (index, (value, expected)) in args.iter().zip(&signature.params).enumerate() {
        validate_runtime_value_at(
            program,
            value,
            Some(*expected),
            format!("suspension.host_call.args[{index}]"),
        )?;
        if !value.ownership().permits_copy() {
            return Err(FiberStateError::InvalidRuntimeValue {
                path: format!("suspension.host_call.args[{index}]"),
                reason: "host call arguments must be deep-Copy values".to_owned(),
            });
        }
    }
    match (signature.result, destination) {
        (None, None) => Ok(()),
        (Some(_), Some(destination)) if destination.index() < frame.registers.len() => Ok(()),
        _ => Err(FiberStateError::InvalidFrame),
    }
}

fn validate_terminal(
    program: &AwbcProgram,
    state: &FiberState,
    terminal: &FiberTerminalValue,
) -> Result<(), FiberStateError> {
    match terminal {
        FiberTerminalValue::Returned(Some(value)) => {
            let expected = match state.root {
                AwbcFiberRoot::Program(id) => {
                    let binding = program
                        .pure_program_binding(id)
                        .ok_or(FiberStateError::UnknownProgram(id))?;
                    let signature = program
                        .functions
                        .get(binding.function.index())
                        .and_then(|function| program.signatures.get(function.signature.index()))
                        .ok_or(FiberStateError::InvalidFrame)?;
                    Some(signature.result.ok_or(FiberStateError::InvalidFrame)?)
                }
                AwbcFiberRoot::Entry(_) | AwbcFiberRoot::Function(_) | AwbcFiberRoot::Empty => None,
            };
            if let Some(expected) = expected
                && !state.frames.first().map_or_else(
                    || {
                        program
                            .runtime_types
                            .get(expected.index())
                            .is_some_and(|row| row.scope().is_root())
                            && runtime_value_matches_type(program, value, expected, 0)
                    },
                    |frame| frame.value_matches_type(program, value, expected),
                )
            {
                return Err(FiberStateError::ReturnValueMismatch);
            }
            validate_runtime_value_at(program, value, None, "terminal.returned".to_owned())
        }
        FiberTerminalValue::DialogueResultSelected(value) => {
            if state.frames.len() != 1 {
                return Err(FiberStateError::InvalidFrame);
            }
            let function = state
                .frames
                .last()
                .and_then(|frame| program.functions.get(frame.function.index()))
                .ok_or(FiberStateError::InvalidFrame)?;
            if !matches!(
                function.kind,
                crate::awbc::schema::AwbcFunctionKind::LineTask
                    | crate::awbc::schema::AwbcFunctionKind::LineCancellationHandler
            ) {
                return Err(FiberStateError::InvalidFrame);
            }
            validate_runtime_value_at(
                program,
                value,
                None,
                "terminal.dialogue_result_selected".to_owned(),
            )
        }
        FiberTerminalValue::Returned(None) | FiberTerminalValue::Cancelled => Ok(()),
        FiberTerminalValue::Trapped(trap) => {
            if trap
                .source_map
                .is_some_and(|source_map| program.source_map.get(source_map.index()).is_none())
            {
                return Err(FiberStateError::InvalidFrame);
            }
            Ok(())
        }
    }
}

fn validate_stream(
    program: &AwbcProgram,
    stream: &FiberStreamState,
    path: &str,
) -> Result<(), FiberStateError> {
    let plan = program
        .stream_plans
        .get(stream.plan.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    for (index, value) in stream.queue.iter().enumerate() {
        validate_runtime_value_at(
            program,
            value,
            Some(plan.item_type),
            format!("{path}.queue[{index}]"),
        )?;
        crate::stream::RuntimeStreamYieldCopyProof::inspect(value).map_err(|error| {
            FiberStateError::InvalidRuntimeValue {
                path: format!("{path}.queue[{index}]"),
                reason: error.to_string(),
            }
        })?;
    }
    Ok(())
}

fn function_owns_block(function: &super::schema::AwbcFunction, block: AwbcBlockId) -> bool {
    let Some(end) = function.blocks.checked_end() else {
        return false;
    };
    block.0 >= function.blocks.start && block.0 < end
}

impl FiberFrame {
    pub fn new(
        instance: RuntimeFrameInstanceId,
        program: &AwbcProgram,
        function: AwbcFunctionId,
        return_to: Option<FiberReturnPoint>,
    ) -> Result<Self, FiberStateError> {
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let layout = program
            .frame_layouts
            .get(function_record.frame_layout.index())
            .ok_or(FiberStateError::UnknownFrameLayout(
                function_record.frame_layout.0,
            ))?;
        Ok(Self {
            instance,
            function,
            type_instantiation: None,
            layout: function_record.frame_layout,
            return_to,
            registers: vec![RuntimePlaceStorage::default(); layout.slots.len()],
            format: None,
            format_attempts: Vec::new(),
            root_cleanups: Vec::new(),
            root_defers: Vec::new(),
            scopes: Vec::with_capacity(layout.max_scope_depth as usize),
        })
    }

    fn has_pending_cleanup(&self) -> bool {
        !self.root_cleanups.is_empty()
            || !self.root_defers.is_empty()
            || self.scopes.iter().any(|scope| {
                !scope.cleanups.is_empty()
                    || !scope.defers.is_empty()
                    || !scope.defer_releasing.is_empty()
                    || scope.defer_inflight.is_some()
                    || scope.defer_exit.is_some()
            })
    }

    pub fn bind_positional_arguments(
        &mut self,
        program: &AwbcProgram,
        args: &[RuntimeValue],
    ) -> Result<(), FiberStateError> {
        let function = program
            .functions
            .get(self.function.index())
            .ok_or(FiberStateError::UnknownFunction(self.function.0))?;
        let signature = program
            .signatures
            .get(function.signature.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let layout = program
            .frame_layouts
            .get(self.layout.index())
            .ok_or(FiberStateError::UnknownFrameLayout(self.layout.0))?;
        let parameters = layout
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.role == AwbcFrameSlotRole::Parameter)
            .collect::<Vec<_>>();
        if parameters.len() != signature.params.len() || args.len() != parameters.len() {
            return Err(FiberStateError::ArgumentCount {
                expected: parameters.len(),
                actual: args.len(),
            });
        }
        let references = args.iter().collect::<Vec<_>>();
        let type_instantiation = self.instantiate_arguments(program, &references)?;
        super::vm::validate_function_input_ownership_values(program, self.function, &references)
            .map_err(|error| FiberStateError::InvalidFunctionInputOwnership {
                reason: error.to_string(),
            })?;
        for (position, value) in args.iter().enumerate() {
            if !value.ownership().permits_copy() {
                return Err(FiberStateError::ArgumentNotCopyable { position });
            }
        }
        let mut next = self.registers.clone();
        for (position, ((register, slot), value)) in parameters.iter().zip(args).enumerate() {
            let expected = signature.params[position];
            if slot.ty != expected
                || !Self::value_matches_instantiation(
                    program,
                    type_instantiation.as_ref(),
                    value,
                    expected,
                )
            {
                return Err(FiberStateError::ArgumentType {
                    name: slot
                        .name
                        .and_then(|id| program.strings.get(id.index()).cloned())
                        .unwrap_or_else(|| format!("${position}")),
                    expected: runtime_type_label(program, expected),
                    actual: runtime_value_type_label(value),
                });
            }
            next[*register] = value.clone().into();
        }
        self.registers = next;
        self.type_instantiation = type_instantiation.map(std::sync::Arc::new);
        Ok(())
    }

    pub(crate) fn bind_positional_arguments_owned(
        &mut self,
        program: &AwbcProgram,
        args: Vec<RuntimeValue>,
    ) -> Result<(), FiberStateError> {
        let function = program
            .functions
            .get(self.function.index())
            .ok_or(FiberStateError::UnknownFunction(self.function.0))?;
        let signature = program
            .signatures
            .get(function.signature.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let layout = program
            .frame_layouts
            .get(self.layout.index())
            .ok_or(FiberStateError::UnknownFrameLayout(self.layout.0))?;
        let parameters = layout
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.role == AwbcFrameSlotRole::Parameter)
            .collect::<Vec<_>>();
        if parameters.len() != signature.params.len() || args.len() != parameters.len() {
            return Err(FiberStateError::ArgumentCount {
                expected: parameters.len(),
                actual: args.len(),
            });
        }
        let references = args.iter().collect::<Vec<_>>();
        let type_instantiation = self.instantiate_arguments(program, &references)?;
        for (position, ((_, slot), value)) in parameters.iter().zip(&args).enumerate() {
            let expected = signature.params[position];
            if slot.ty != expected
                || !Self::value_matches_instantiation(
                    program,
                    type_instantiation.as_ref(),
                    value,
                    expected,
                )
            {
                return Err(FiberStateError::ArgumentType {
                    name: slot
                        .name
                        .and_then(|id| program.strings.get(id.index()).cloned())
                        .unwrap_or_else(|| format!("${position}")),
                    expected: runtime_type_label(program, expected),
                    actual: runtime_value_type_label(value),
                });
            }
        }
        super::vm::validate_function_input_ownership_values(program, self.function, &references)
            .map_err(|error| FiberStateError::InvalidFunctionInputOwnership {
                reason: error.to_string(),
            })?;
        let mut next = self.registers.clone();
        for ((register, _), value) in parameters.into_iter().zip(args) {
            next[register] = value.into();
        }
        self.registers = next;
        self.type_instantiation = type_instantiation.map(std::sync::Arc::new);
        Ok(())
    }

    fn instantiate_arguments(
        &self,
        program: &AwbcProgram,
        values: &[&RuntimeValue],
    ) -> Result<Option<crate::program_types::RuntimeFunctionEffectInstantiation>, FiberStateError>
    {
        let function = program
            .functions
            .get(self.function.index())
            .ok_or(FiberStateError::UnknownFunction(self.function.0))?;
        Self::instantiate_context(program, function.type_context, values)
    }

    fn instantiate_context(
        program: &AwbcProgram,
        context: Option<AwbcTypeId>,
        values: &[&RuntimeValue],
    ) -> Result<Option<crate::program_types::RuntimeFunctionEffectInstantiation>, FiberStateError>
    {
        context
            .map(|context| {
                program.instantiate_function_effects(context, values).ok_or(
                    FiberStateError::InvalidRuntimeValue {
                        path: "function arguments".to_owned(),
                        reason: "arguments have no joint declaration effect instantiation"
                            .to_owned(),
                    },
                )
            })
            .transpose()
    }

    fn value_matches_instantiation(
        program: &AwbcProgram,
        instantiation: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        value: &RuntimeValue,
        expected: super::schema::AwbcTypeId,
    ) -> bool {
        instantiation.map_or_else(
            || runtime_value_matches_type(program, value, expected, 0),
            |binding| binding.value_matches(program, expected, value),
        )
    }

    pub(crate) fn value_matches_type(
        &self,
        program: &AwbcProgram,
        value: &RuntimeValue,
        expected: super::schema::AwbcTypeId,
    ) -> bool {
        Self::value_matches_instantiation(
            program,
            self.type_instantiation.as_deref(),
            value,
            expected,
        )
    }

    pub(crate) fn value_view_matches_type(
        &self,
        program: &AwbcProgram,
        value: crate::value::RuntimeValueView<'_>,
        expected: AwbcTypeId,
    ) -> bool {
        self.type_instantiation.as_deref().map_or_else(
            || super::vm::runtime_value_view_matches_type(program, value, expected, 0),
            |binding| binding.value_view_matches(program, expected, value),
        )
    }

    pub(crate) fn take_positional_argument_storage(
        &mut self,
        program: &AwbcProgram,
    ) -> Result<Vec<RuntimePlaceStorage<RuntimeValue>>, FiberStateError> {
        let registers = self.validated_positional_argument_registers(program)?;
        Ok(registers
            .into_iter()
            .map(|register| std::mem::take(&mut self.registers[register.index()]))
            .collect())
    }

    pub(crate) fn positional_argument_storage<'a>(
        &'a self,
        program: &AwbcProgram,
    ) -> Result<Vec<&'a RuntimePlaceStorage<RuntimeValue>>, FiberStateError> {
        Ok(self
            .validated_positional_argument_registers(program)?
            .into_iter()
            .map(|register| &self.registers[register.index()])
            .collect())
    }

    fn validated_positional_argument_registers(
        &self,
        program: &AwbcProgram,
    ) -> Result<Vec<AwbcRegisterId>, FiberStateError> {
        let function = program
            .functions
            .get(self.function.index())
            .ok_or(FiberStateError::UnknownFunction(self.function.0))?;
        let signature = program
            .signatures
            .get(function.signature.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let layout = program
            .frame_layouts
            .get(self.layout.index())
            .ok_or(FiberStateError::UnknownFrameLayout(self.layout.0))?;
        let parameters = layout
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.role == AwbcFrameSlotRole::Parameter)
            .collect::<Vec<_>>();
        if parameters.len() != signature.params.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        parameters
            .into_iter()
            .zip(&signature.params)
            .map(|((index, slot), expected)| {
                if slot.ty != *expected {
                    return Err(FiberStateError::InvalidFrame);
                }
                let register = u32::try_from(index)
                    .map(AwbcRegisterId)
                    .map_err(|_| FiberStateError::InvalidFrame)?;
                let storage =
                    self.registers
                        .get(index)
                        .ok_or(FiberStateError::RegisterOutOfBounds {
                            register: register.0,
                            layout: self.layout.0,
                        })?;
                validate_place_storage_at(
                    program,
                    self.type_instantiation.as_deref(),
                    storage,
                    *expected,
                    format!("parameter_storage[{index}]"),
                    0,
                )?;
                Ok(register)
            })
            .collect()
    }
    pub fn register(&self, register: AwbcRegisterId) -> Result<&RuntimeValue, FiberStateError> {
        self.registers
            .get(register.index())
            .and_then(RuntimePlaceStorage::as_ref)
            .ok_or(FiberStateError::RegisterOutOfBounds {
                register: register.0,
                layout: self.layout.0,
            })
    }

    pub fn set_register(
        &mut self,
        register: AwbcRegisterId,
        value: RuntimeValue,
    ) -> Result<(), FiberStateError> {
        let slot = self.registers.get_mut(register.index()).ok_or(
            FiberStateError::RegisterOutOfBounds {
                register: register.0,
                layout: self.layout.0,
            },
        )?;
        *slot = value.into();
        Ok(())
    }

    pub fn clear_register(&mut self, register: AwbcRegisterId) -> Result<(), FiberStateError> {
        let slot = self.registers.get_mut(register.index()).ok_or(
            FiberStateError::RegisterOutOfBounds {
                register: register.0,
                layout: self.layout.0,
            },
        )?;
        *slot = RuntimePlaceStorage::default();
        Ok(())
    }

    pub fn take_register(
        &mut self,
        register: AwbcRegisterId,
    ) -> Result<RuntimeValue, FiberStateError> {
        self.registers
            .get_mut(register.index())
            .ok_or(FiberStateError::RegisterOutOfBounds {
                register: register.0,
                layout: self.layout.0,
            })?
            .take()
            .ok_or(FiberStateError::RegisterOutOfBounds {
                register: register.0,
                layout: self.layout.0,
            })
    }
}

pub(crate) fn runtime_value_matches_type(
    program: &AwbcProgram,
    value: &RuntimeValue,
    ty: AwbcTypeId,
    depth: usize,
) -> bool {
    super::vm::runtime_value_view_matches_type(program, value.view(), ty, depth)
}

pub(crate) fn runtime_variant_identity(
    program: &AwbcProgram,
    semantic_identity: RuntimeSemanticTypeId,
    owner: &AwbcVariantIdentity,
) -> Option<RuntimeVariantIdentity> {
    match owner {
        AwbcVariantIdentity::Nominal { public_id, layout } => {
            Some(RuntimeVariantIdentity::Nominal {
                nominal: RuntimeNominalTypeId::try_new(
                    program.strings.get(public_id.index())?.clone(),
                )
                .ok()?,
                semantic_identity,
                layout: crate::entry::TypeLayoutHash::from_bytes(*layout),
            })
        }
        AwbcVariantIdentity::Builtin(owner) => Some(RuntimeVariantIdentity::Builtin(*owner)),
    }
}

fn runtime_type_label(program: &AwbcProgram, ty: AwbcTypeId) -> String {
    program
        .runtime_types
        .get(ty.index())
        .map_or_else(|| format!("type#{}", ty.0), |ty| format!("{ty:?}"))
}

fn runtime_value_type_label(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "unit",
        RuntimeValue::Bool(_) => "bool",
        RuntimeValue::Int(_) => "int",
        RuntimeValue::UInt(_) => "uint",
        RuntimeValue::F32(_) => "f32",
        RuntimeValue::F64(_) => "f64",
        RuntimeValue::MatrixF32(_) => "matrix<f32>",
        RuntimeValue::MatrixF64(_) => "matrix<f64>",
        RuntimeValue::TensorF32(_) => "tensor<f32>",
        RuntimeValue::TensorF64(_) => "tensor<f64>",
        RuntimeValue::String(_) => "string",
        RuntimeValue::Color(_) => "color",
        RuntimeValue::NeedHandle(_) => "need",
        RuntimeValue::Char(_) => "char",
        RuntimeValue::Duration(_) => "duration",
        RuntimeValue::Progress(_) => "progress",
        RuntimeValue::Range(_) => "range",
        RuntimeValue::Iterator(_) => "iterator",
        RuntimeValue::EntityRef(_) => "entity",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::Seq(_) => "sequence",
        RuntimeValue::Record(_) => "record",
        RuntimeValue::NominalRecord(record) => record.type_id().as_str(),
        RuntimeValue::Opaque(_) => "opaque value",
        RuntimeValue::Reduction(_) => "reduction",
        RuntimeValue::Agent(value) => value.label(),
        RuntimeValue::Callable(_) => "callable",
        RuntimeValue::Variant { .. } => "variant",
    }
    .to_owned()
}

#[cfg(test)]
#[allow(clippy::default_trait_access, clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::awbc::schema::{
        AwbcBlock, AwbcEntry, AwbcEntryKind, AwbcFlowBinding, AwbcFlowExecutable,
        AwbcFormatOperand, AwbcFrameLayout, AwbcFrameSlot, AwbcFrameSlotRole, AwbcFunction,
        AwbcFunctionFlags, AwbcFunctionInputOwnership, AwbcFunctionKind, AwbcResumePoint,
        AwbcRuntimeType, AwbcSafePointKind, AwbcSignature, AwbcStringId, AwbcTableRange,
        AwbcTerminator,
    };

    fn zero_parameter_entry_program() -> AwbcProgram {
        let mut program = AwbcProgram::default();
        program.strings = vec!["entry".to_owned()];
        program.signatures.push(AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: Default::default(),
        });
        program.frame_layouts.push(AwbcFrameLayout {
            scopes: Vec::new(),
            slots: Vec::new(),
            max_scope_depth: 0,
        });
        program.functions.push(AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: Default::default(),
            type_context: None,
            input_ownership: Vec::new(),
            frame_layout: Default::default(),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: Default::default(),
            flags: AwbcFunctionFlags::default(),
        });
        program.flow_bindings.push(AwbcFlowBinding {
            flow: crate::plan::FlowRuntimeId::from_checked_declaration_digest(
                [0x34; 32],
                "flow.main",
            )
            .expect("test checked Flow identity"),
            function: Default::default(),
        });
        program.blocks.push(AwbcBlock {
            owner: Default::default(),
            instructions: AwbcTableRange::default(),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::Return,
            source_map: None,
        });
        program.entries.push(AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Cli,
            target: AwbcEntryTarget::Function {
                function: Default::default(),
            },
            roles: crate::entry::RuntimeEntryRoles::None,
        });
        program
    }

    fn one_unit_parameter_entry_program() -> AwbcProgram {
        let mut program = zero_parameter_entry_program();
        program.runtime_types.push(AwbcRuntimeType::unit());
        program.signatures[0].params.push(AwbcTypeId(0));
        program.functions[0]
            .input_ownership
            .push(AwbcFunctionInputOwnership::default());
        program.frame_layouts[0].slots.push(AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        });
        program
    }

    fn format_entry_program() -> AwbcProgram {
        let mut program = zero_parameter_entry_program();
        program.runtime_types.push(AwbcRuntimeType::unit());
        program.runtime_types.push(AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x51; 32]),
            AwbcRuntimeTypeShape::String,
        ));
        program.frame_layouts[0].slots.push(AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: AwbcFrameSlotRole::Local,
            scope_depth: 0,
        });
        program.frame_layouts[0].slots.push(AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(1),
            role: AwbcFrameSlotRole::Local,
            scope_depth: 0,
        });
        program.frame_layouts.push(AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        });
        program.signatures.push(AwbcSignature {
            params: vec![AwbcTypeId(0)],
            result: Some(AwbcTypeId(0)),
            effects: Default::default(),
        });
        program.signatures.push(AwbcSignature {
            params: vec![AwbcTypeId(0)],
            result: Some(AwbcTypeId(1)),
            effects: Default::default(),
        });
        for (function_id, signature_id) in [(1, 1), (2, 2)] {
            program.functions.push(AwbcFunction {
                public_id: None,
                kind: AwbcFunctionKind::Synthetic,
                signature: AwbcSignatureId(signature_id),
                type_context: None,
                input_ownership: vec![AwbcFunctionInputOwnership::default()],
                frame_layout: AwbcFrameLayoutId(1),
                blocks: AwbcTableRange::new(function_id, 1),
                entry_block: AwbcBlockId(function_id),
                flags: AwbcFunctionFlags::default(),
            });
            program.blocks.push(AwbcBlock {
                owner: AwbcFunctionId(function_id),
                instructions: AwbcTableRange::default(),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::Return,
                source_map: None,
            });
        }
        program.instructions.push(AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(1),
            template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .unwrap(),
            attempt: None,
            attempt_operands: Vec::new(),
            project_method: None,
            project_option: false,
            project_result: None,
            operands: vec![
                AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Value,
                    function: AwbcFunctionId(1),
                    captures: vec![AwbcRegisterId(0)],
                },
                AwbcFormatOperand {
                    parameter: crate::value::RuntimeFmtParameterId::Style,
                    function: AwbcFunctionId(2),
                    captures: vec![AwbcRegisterId(0)],
                },
            ],
        });
        program.blocks[0].instructions = AwbcTableRange::new(0, 1);
        program
    }

    fn format_operand_return(site: FiberCursor, ordinal: usize) -> FiberReturnPoint {
        FiberReturnPoint {
            cursor: site,
            destination: None,
            continuation: FiberReturnContinuation::FormatOperand { site, ordinal },
        }
    }

    fn character_dialogue_target_program() -> (AwbcProgram, crate::value::RuntimeOpaqueValue) {
        let mut program = zero_parameter_entry_program();
        program.strings.push("std.character_dialogue".to_owned());
        let declared_identity = RuntimeSemanticTypeId::from_bytes([0x41; 32]);
        program.runtime_types.push(AwbcRuntimeType::new(
            declared_identity,
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(1),
                admission: crate::pattern::RuntimeOpaqueTypeAdmission::ProducerWide,
                value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments: Vec::new(),
            },
        ));
        let producer = crate::value::RuntimeCharacterDialogueProducerId::get();
        let value_identity = RuntimeSemanticTypeId::from_bytes([0x42; 32]);
        let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
            producer,
            value_identity,
            crate::value::RuntimeOpaqueValueClass::Plain,
            crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
        );
        let target = crate::value::RuntimeOpaqueValue::new_exact(&owner, RuntimeValue::Unit);
        (program, target)
    }

    fn bundle_image_handle_value() -> RuntimeValue {
        let artifact = crate::value::RuntimeBundleAssetArtifactDigest::try_from_bytes([0x31; 32])
            .expect("fixture artifact digest is nonzero");
        let context = crate::value::RuntimeBundleAssetContext::new(
            crate::task::GenerationId::new(0),
            artifact,
        );
        let resource = crate::value::RuntimeBundleAssetResourceId::try_new("asset.bg.room")
            .expect("fixture asset identity is canonical");
        let content = crate::value::RuntimeAssetContentDigest::try_for_bytes(b"image")
            .expect("fixture content digest fits");
        let binding = crate::value::RuntimeBundleAssetBinding::try_new(context, resource, content)
            .expect("fixture asset binding is valid");
        crate::value::RuntimeImageHandleValue::from_binding(binding)
            .into_runtime_value()
            .expect("fixture ImageHandle is standard-owned")
    }

    #[test]
    fn fiber_value_visitor_finds_asset_handles_nested_in_register_values() {
        let program = zero_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, crate::awbc::schema::AwbcEntryId(0), 0, 64)
            .expect("fixture fiber starts");
        fiber.frames[0]
            .registers
            .push(RuntimeValue::Tuple(vec![bundle_image_handle_value()]).into());
        let mut roles = Vec::new();

        fiber
            .visit_runtime_values(|value| {
                if let Some(role) = crate::value::runtime_bundle_asset_opaque_role(value) {
                    roles.push(role);
                }
                Ok::<_, ()>(())
            })
            .expect("fiber runtime values visit");

        assert_eq!(
            roles,
            vec![crate::value::RuntimeBundleAssetOpaqueRole::ImageHandle]
        );
    }

    #[test]
    fn dialogue_target_is_admitted_by_its_selected_awbc_type_row() {
        let (program, target) = character_dialogue_target_program();
        let target_value = RuntimeValue::Opaque(target.clone());
        assert!(
            program
                .opaque_owner(AwbcTypeId(2))
                .expect("target type reifies")
                .is_some()
        );
        assert!(
            program
                .accepts_value(
                    AwbcTypeId(2),
                    &target_value,
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .is_ok()
        );
        assert!(dialogue_target_matches_program(
            &program,
            AwbcTypeId(2),
            &target_value,
        ));
        assert!(!dialogue_target_matches_program(
            &program,
            AwbcTypeId(0),
            &RuntimeValue::Opaque(target),
        ));
        let foreign_owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
            crate::pattern::RuntimeOpaqueTypeProducerId::try_new("fixture.foreign")
                .expect("foreign producer ID"),
            RuntimeSemanticTypeId::from_bytes([0x43; 32]),
            crate::value::RuntimeOpaqueValueClass::Plain,
            crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
        );
        let foreign =
            crate::value::RuntimeOpaqueValue::new_exact(&foreign_owner, RuntimeValue::Unit);
        assert!(!dialogue_target_matches_program(
            &program,
            AwbcTypeId(2),
            &RuntimeValue::Opaque(foreign),
        ));
    }

    #[test]
    fn dialogue_fiber_suspension_snapshot_preserves_exact_target_and_type() {
        let (program, target) = character_dialogue_target_program();
        let reason = FiberSuspensionReason::Dialogue {
            target: Some(target.clone()),
            target_type: AwbcTypeId(2),
            content: AwbcContentUnitId(0),
            values: Box::new([]),
            effects: Box::new([]),
            line_task_captures: Box::new([]),
            result: AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
        };
        let snapshot = AwbcFiberSuspensionReasonSnapshot::from_live(&reason)
            .expect("dialogue target snapshots explicitly");
        let encoded = serde_json::to_vec(&snapshot).expect("snapshot serializes");
        let decoded = serde_json::from_slice::<AwbcFiberSuspensionReasonSnapshot>(&encoded)
            .expect("snapshot decodes");
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(program));
        let restored = decoded
            .into_live(&owner)
            .expect("target restores under selected AWBC owner");
        assert_eq!(restored, reason);
        let FiberSuspensionReason::Dialogue {
            target: restored_target,
            target_type,
            ..
        } = restored
        else {
            unreachable!("fixture is a dialogue suspension")
        };
        assert_eq!(restored_target, Some(target));
        assert_eq!(target_type, AwbcTypeId(2));
    }

    #[test]
    fn fiber_snapshot_validation_enforces_agent_structural_nesting() {
        fn nested_predicate(depth: usize) -> RuntimeValue {
            let mut predicate = crate::value::RuntimeAgentPredicate::DiagnosticsHasError;
            for _ in 0..depth {
                predicate = crate::value::RuntimeAgentPredicate::Not {
                    predicate: Box::new(predicate),
                };
            }
            RuntimeValue::Agent(crate::value::RuntimeAgentValue::Predicate(predicate))
        }

        assert!(
            validate_nested_runtime_value(&AwbcProgram::default(), &nested_predicate(64), 0)
                .is_ok()
        );
        assert!(matches!(
            validate_nested_runtime_value(&AwbcProgram::default(), &nested_predicate(65), 0),
            Err(FiberStateError::InvalidRuntimeCallable { reason })
                if reason.contains("nesting exceeds 64")
        ));
    }

    #[test]
    fn fiber_snapshot_validation_rejects_invalid_cursor_shape() {
        let program = zero_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        fiber.validate_for_program(&program).unwrap();

        fiber.cursor.instruction_offset = 1;
        assert_eq!(
            fiber.validate_for_program(&program),
            Err(FiberStateError::InvalidFrame)
        );
    }

    #[test]
    fn bind_function_argument_values_commits_valid_positional_values() {
        let program = one_unit_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();

        fiber
            .bind_function_argument_values(&program, &[RuntimeValue::Unit])
            .expect("valid positional value binds");

        assert_eq!(
            fiber.active_frame().unwrap().registers,
            vec![RuntimePlaceStorage::from(RuntimeValue::Unit)]
        );
    }

    #[test]
    fn bind_function_argument_values_rejects_arity_and_type_without_mutation() {
        let program = one_unit_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        let before = fiber.checkpoint().unwrap();

        assert_eq!(
            fiber.bind_function_argument_values(&program, &[]),
            Err(FiberStateError::ArgumentCount {
                expected: 1,
                actual: 0,
            })
        );
        assert_eq!(fiber.checkpoint().unwrap(), before);

        assert!(matches!(
            fiber.bind_function_argument_values(&program, &[RuntimeValue::Bool(true)]),
            Err(FiberStateError::ArgumentType { .. })
        ));
        assert_eq!(fiber.checkpoint().unwrap(), before);
    }

    #[test]
    fn await_progress_resume_returns_the_owned_need_to_its_rewait_register() {
        let mut program = one_unit_parameter_entry_program();
        program.runtime_types = vec![
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x41; 32]),
                AwbcRuntimeTypeShape::Need(AwbcTypeId(1)),
            ),
            AwbcRuntimeType::unit(),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x42; 32]),
                AwbcRuntimeTypeShape::Progress,
            ),
        ];
        program.frame_layouts[0].slots.push(AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(2),
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        });
        program.functions[0].blocks = AwbcTableRange::new(0, 4);
        program.blocks[0].terminator = AwbcTerminator::Jump {
            target: AwbcBlockId(1),
        };
        program.blocks[0].safe_point = AwbcSafePointKind::FlowEntry;
        program.blocks.extend([
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::default(),
                terminator: AwbcTerminator::Await {
                    handle: AwbcRegisterId(0),
                    binding: None,
                    observer: Some(crate::awbc::schema::AwbcAwaitObserverResume {
                        destination: AwbcRegisterId(1),
                        resume: AwbcResumePointId(1),
                    }),
                    resume: AwbcResumePointId(0),
                },
                safe_point: AwbcSafePointKind::LoopBackedge,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::default(),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::Await,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::default(),
                terminator: AwbcTerminator::Jump {
                    target: AwbcBlockId(1),
                },
                safe_point: AwbcSafePointKind::Await,
                source_map: None,
            },
        ]);
        program.resume_points = vec![
            AwbcResumePoint {
                function: AwbcFunctionId(0),
                block: AwbcBlockId(2),
                frame_layout: AwbcFrameLayoutId(0),
                kind: AwbcSafePointKind::Await,
            },
            AwbcResumePoint {
                function: AwbcFunctionId(0),
                block: AwbcBlockId(3),
                frame_layout: AwbcFrameLayoutId(0),
                kind: AwbcSafePointKind::Await,
            },
        ];
        program.flow_executables.push(AwbcFlowExecutable {
            metadata: crate::entry::RuntimeFlowExecutable {
                flow: program.flow_bindings[0].flow.clone(),
                contract: crate::entry::FlowContractHash::from_bytes([0x51; 32]),
                controller: None,
            },
            function: AwbcFunctionId(0),
        });
        program
            .verify(Default::default(), Default::default())
            .expect("observer backedge has its Need handle on the progress edge");

        let need = crate::tests::reusable_need_with_outcome(
            "need.progress",
            crate::task::TaskOutcomeContract::program(program.runtime_types[1].semantic_identity()),
        );
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
        fiber
            .bind_function_argument_values_owned(
                &program,
                vec![RuntimeValue::NeedHandle(need.clone())],
            )
            .unwrap();
        let first = crate::awbc::vm::step(&program, &mut fiber, Default::default()).unwrap();
        assert!(matches!(
            first.exit,
            crate::awbc::vm::VmExit::Suspended(FiberSuspensionReason::Await { .. })
        ));
        assert!(
            fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(0))
                .is_err()
        );
        let missing_progress = fiber.checkpoint().unwrap();
        assert!(matches!(
            fiber.resume_await_observer_at(&program, AwbcResumePointId(1)),
            Err(FiberStateError::InvalidRuntimeValue { .. })
        ));
        assert_eq!(fiber.checkpoint().unwrap(), missing_progress);

        fiber
            .active_frame_mut()
            .unwrap()
            .set_register(
                AwbcRegisterId(0),
                RuntimeValue::NeedHandle(crate::tests::reusable_need("need.occupied")),
            )
            .unwrap();
        fiber
            .active_frame_mut()
            .unwrap()
            .set_register(
                AwbcRegisterId(1),
                RuntimeValue::Progress(crate::value::Progress::new(0.5).unwrap()),
            )
            .unwrap();
        let occupied = fiber.checkpoint().unwrap();
        assert!(matches!(
            fiber.resume_await_observer_at(&program, AwbcResumePointId(1)),
            Err(FiberStateError::InvalidRuntimeValue { .. })
        ));
        assert_eq!(fiber.checkpoint().unwrap(), occupied);
        fiber
            .active_frame_mut()
            .unwrap()
            .clear_register(AwbcRegisterId(0))
            .unwrap();

        fiber
            .resume_await_observer_at(&program, AwbcResumePointId(1))
            .unwrap();
        assert_eq!(
            fiber.active_frame().unwrap().register(AwbcRegisterId(0)),
            Ok(&RuntimeValue::NeedHandle(need.clone()))
        );
        assert!(fiber.suspension.is_none());

        let second = crate::awbc::vm::step(&program, &mut fiber, Default::default()).unwrap();
        assert!(matches!(
            second.exit,
            crate::awbc::vm::VmExit::Suspended(FiberSuspensionReason::Await {
                target: FiberAwaitTarget::Need { need: id, .. },
                ..
            }) if id == need
        ));
        assert!(
            fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(0))
                .is_err()
        );
        fiber.resume_at(&program, AwbcResumePointId(0)).unwrap();
        assert!(
            fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(0))
                .is_err()
        );
    }

    #[test]
    fn commit_yielded_instruction_advances_exact_observation() {
        let program = zero_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        let observed = fiber.cursor;

        fiber
            .commit_yielded_instruction(observed)
            .expect("exact running cursor commits");

        assert_eq!(fiber.cursor.instruction_offset, 1);
    }

    #[test]
    fn commit_yielded_instruction_rejects_stale_cursor_without_mutation() {
        let program = zero_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        let stale = FiberCursor {
            instruction_offset: 1,
            ..fiber.cursor
        };
        let before = fiber.checkpoint().unwrap();

        assert!(matches!(
            fiber.commit_yielded_instruction(stale),
            Err(FiberStateError::StaleCursor { .. })
        ));
        assert_eq!(fiber.checkpoint().unwrap(), before);
    }

    #[test]
    fn commit_yielded_instruction_rejects_offset_overflow_without_mutation() {
        let program = zero_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        fiber.cursor.instruction_offset = u32::MAX;
        let observed = fiber.cursor;
        let before = fiber.checkpoint().unwrap();

        assert!(matches!(
            fiber.commit_yielded_instruction(observed),
            Err(FiberStateError::InstructionOffsetOverflow { cursor }) if cursor == observed
        ));
        assert_eq!(fiber.checkpoint().unwrap(), before);
    }

    #[test]
    fn format_operand_results_survive_snapshot_and_visit() {
        let program = format_entry_program();
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
        fiber.frames[0].registers[0] = RuntimeValue::Unit.into();
        let site = fiber.cursor;
        fiber
            .begin_format_content(&program, &RuntimeFormatContext::default())
            .unwrap();
        fiber
            .push_call_frame_with_continuation(
                &program,
                AwbcFunctionId(1),
                format_operand_return(site, 0),
                &[RuntimeValue::Unit],
            )
            .unwrap();
        fiber.validate_for_program(&program).unwrap();
        assert!(
            !fiber
                .finish_return(&program, Some(RuntimeValue::Unit))
                .unwrap()
        );
        assert_eq!(fiber.cursor, site);
        assert_eq!(fiber.format_content_state().unwrap().next_operand(), 1);

        fiber
            .push_call_frame_with_continuation(
                &program,
                AwbcFunctionId(2),
                format_operand_return(site, 1),
                &[RuntimeValue::Unit],
            )
            .unwrap();
        assert!(
            !fiber
                .finish_return(&program, Some(RuntimeValue::String("long".to_owned())))
                .unwrap()
        );
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(program.clone()));
        let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
        let encoded = serde_json::to_vec(&snapshot).unwrap();
        let decoded = serde_json::from_slice::<AwbcFiberStateSnapshot>(&encoded).unwrap();
        let mut restored = decoded.into_live_for_program(&owner).unwrap();
        restored.validate_for_program(&program).unwrap();
        let mut strings = Vec::new();
        restored
            .visit_runtime_values(|value| {
                if let RuntimeValue::String(value) = value {
                    strings.push(value.clone());
                }
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(strings, ["long"]);
        let mut formatter_values = Vec::new();
        restored
            .visit_formatter_operand_values(|frame, site, ordinal, value| {
                formatter_values.push((frame, site, ordinal, value.clone()));
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(
            formatter_values,
            [
                (restored.frames[0].instance, site, 0, RuntimeValue::Unit),
                (
                    restored.frames[0].instance,
                    site,
                    1,
                    RuntimeValue::String("long".to_owned())
                ),
            ]
        );
        let complete = restored.take_completed_format_content(&program).unwrap();
        assert_eq!(complete.values().len(), 2);
        assert_eq!(complete.first_recoverable(), None);
        assert!(restored.active_frame().unwrap().format.is_none());
    }

    #[test]
    fn format_operand_recovery_unwinds_nested_call_without_consuming_another_operand() {
        let program = format_entry_program();
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
        fiber.frames[0].registers[0] = RuntimeValue::Unit.into();
        let site = fiber.cursor;
        fiber
            .begin_format_content(&program, &RuntimeFormatContext::default())
            .unwrap();
        fiber
            .push_call_frame_with_continuation(
                &program,
                AwbcFunctionId(1),
                format_operand_return(site, 0),
                &[RuntimeValue::Unit],
            )
            .unwrap();
        let nested_site = fiber.cursor;
        fiber
            .push_call_frame_at(
                &program,
                AwbcFunctionId(2),
                FiberReturnPoint::ordinary(nested_site, None),
                &[RuntimeValue::Unit],
            )
            .unwrap();
        assert!(
            fiber
                .recover_format_operand(&program, "operand failed".to_owned())
                .unwrap()
        );
        assert_eq!(fiber.frames.len(), 1);
        assert_eq!(fiber.cursor, site);
        let state = fiber.format_content_state().unwrap();
        assert_eq!(state.next_operand(), 1);
        assert_eq!(state.values(), &[None, None]);
        assert_eq!(state.first_recoverable(), Some("operand failed"));
        fiber.validate_for_program(&program).unwrap();
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(program.clone()));
        let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
        let restored = snapshot.into_live_for_program(&owner).unwrap();
        restored.validate_for_program(&program).unwrap();
        assert_eq!(
            restored.format_content_state().unwrap().first_recoverable(),
            Some("operand failed")
        );
        assert!(
            !fiber
                .recover_format_operand(&program, "unrelated".to_owned())
                .unwrap()
        );
    }

    #[test]
    fn format_operand_recovery_preserves_pending_cleanup_for_normal_unwind() {
        let program = format_entry_program();
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
        fiber.frames[0].registers[0] = RuntimeValue::Unit.into();
        let site = fiber.cursor;
        fiber
            .begin_format_content(&program, &RuntimeFormatContext::default())
            .unwrap();
        fiber
            .push_call_frame_with_continuation(
                &program,
                AwbcFunctionId(1),
                format_operand_return(site, 0),
                &[RuntimeValue::Unit],
            )
            .unwrap();
        fiber
            .active_frame_mut()
            .unwrap()
            .root_cleanups
            .push(FiberScopeCleanup {
                key: "cleanup".to_owned(),
                effect: AwbcEffectPlanId(0),
                args: vec![RuntimeValue::Unit],
            });
        let before = fiber.checkpoint().unwrap();
        assert!(
            !fiber
                .recover_format_operand(&program, "failure".to_owned())
                .unwrap()
        );
        assert_eq!(fiber.checkpoint().unwrap(), before);
        let output = crate::awbc::vm::cancel_fiber(&mut fiber);
        assert!(output.observations.iter().any(|observation| matches!(
            observation,
            crate::awbc::vm::VmObservation::Effect {
                effect: AwbcEffectPlanId(0),
                args,
            } if args == &[RuntimeValue::Unit]
        )));
    }

    #[test]
    fn format_snapshot_rejects_wrong_site_ordinal_function_capture_and_value_type() {
        let program = format_entry_program();
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
        fiber.frames[0].registers[0] = RuntimeValue::Unit.into();
        let site = fiber.cursor;
        fiber
            .begin_format_content(&program, &RuntimeFormatContext::default())
            .unwrap();
        fiber
            .push_call_frame_with_continuation(
                &program,
                AwbcFunctionId(1),
                format_operand_return(site, 0),
                &[RuntimeValue::Unit],
            )
            .unwrap();
        fiber.validate_for_program(&program).unwrap();

        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(program.clone()));
        let mut wrong_ordinal = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
        wrong_ordinal.frames[1]
            .return_to
            .as_mut()
            .unwrap()
            .continuation = AwbcFiberReturnContinuationSnapshot::FormatOperand { site, ordinal: 1 };
        let wrong_ordinal = wrong_ordinal.into_live_for_program(&owner).unwrap();
        assert_eq!(
            wrong_ordinal.validate_for_program(&program),
            Err(FiberStateError::InvalidFrame)
        );

        let mut wrong_site = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
        wrong_site.frames[1]
            .return_to
            .as_mut()
            .unwrap()
            .continuation = AwbcFiberReturnContinuationSnapshot::FormatOperand {
            site: FiberCursor {
                instruction_offset: 1,
                ..site
            },
            ordinal: 0,
        };
        let wrong_site = wrong_site.into_live_for_program(&owner).unwrap();
        assert_eq!(
            wrong_site.validate_for_program(&program),
            Err(FiberStateError::InvalidFrame)
        );

        let mut wrong_function = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
        wrong_function.frames[1].function = AwbcFunctionId(2);
        wrong_function.cursor.function = AwbcFunctionId(2);
        wrong_function.cursor.block = AwbcBlockId(2);
        let wrong_function = wrong_function.into_live_for_program(&owner).unwrap();
        assert_eq!(
            wrong_function.validate_for_program(&program),
            Err(FiberStateError::InvalidFrame)
        );

        let mut wrong_capture_program = program.clone();
        let AwbcInstruction::FormatContent { operands, .. } =
            &mut wrong_capture_program.instructions[0]
        else {
            unreachable!()
        };
        operands[0].captures[0] = AwbcRegisterId(1);
        assert_eq!(
            fiber.validate_for_program(&wrong_capture_program),
            Err(FiberStateError::InvalidFrame)
        );

        let mut wrong_value = fiber;
        wrong_value
            .finish_return(&program, Some(RuntimeValue::Unit))
            .unwrap();
        wrong_value
            .active_frame_mut()
            .unwrap()
            .format
            .as_mut()
            .unwrap()
            .values[0] = Some(RuntimeValue::String("wrong".to_owned()));
        assert!(matches!(
            wrong_value.validate_for_program(&program),
            Err(FiberStateError::InvalidRuntimeValue { .. })
        ));
    }
}
