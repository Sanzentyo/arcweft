//! Executor-neutral AWBC fiber and safe-point state.

use super::schema::{
    AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcDialogueResultTarget, AwbcEffectPlanId,
    AwbcEntryId, AwbcEntryTarget, AwbcFrameLayoutId, AwbcFrameSlotRole, AwbcFunctionId,
    AwbcHostCallId, AwbcPatternId, AwbcProgram, AwbcRegisterId, AwbcResumePointId,
    AwbcRuntimeTypeShape, AwbcScopeId, AwbcSignatureId, AwbcSignedIntKind, AwbcSourceMapId,
    AwbcStreamPlanId, AwbcTaskPlanId, AwbcTrapCode, AwbcTypeId, AwbcUnsignedIntKind,
    AwbcVariantIdentity,
};
use crate::entry::{FlowParameterCoordinate, RuntimeNominalTypeId};
use crate::pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity};
use crate::plan::RuntimeDialogueValueBinding;
use crate::runtime_id::{
    RuntimeFiberInstanceId, RuntimeFrameInstanceId, RuntimeIdCursor, RuntimeIdNamespace,
};
use crate::task::NeedId;
use crate::value::{
    AwbcRuntimeValueSnapshot, RuntimeBinding, RuntimeFlowParameterBinding, RuntimeFunctionBody,
    RuntimeFunctionValue, RuntimeInt, RuntimeIterator, RuntimeSeq, RuntimeUInt, RuntimeValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

type AwbcSaveResult<T> = Result<T, crate::value::AwbcRuntimeValueSnapshotError>;

/// Complete state that may cross compact-VM and compiled-region boundaries.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberState {
    pub instance: RuntimeFiberInstanceId,
    pub next_frame_instance: RuntimeIdCursor,
    pub generation: u64,
    pub entry: AwbcEntryId,
    pub cursor: FiberCursor,
    pub frames: Vec<FiberFrame>,
    pub status: FiberStatus,
    pub suspension: Option<FiberSuspension>,
    pub terminal: Option<FiberTerminalValue>,
    pub budget: FiberBudget,
    pub line_cursor: u64,
    pub streams: Vec<FiberStreamState>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberCursor {
    pub function: AwbcFunctionId,
    pub block: AwbcBlockId,
    /// Offset of the next instruction, or the block length when its terminator is next.
    pub instruction_offset: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberFrame {
    pub instance: RuntimeFrameInstanceId,
    pub function: AwbcFunctionId,
    pub layout: AwbcFrameLayoutId,
    pub return_to: Option<FiberReturnPoint>,
    pub registers: Vec<Option<RuntimeValue>>,
    pub root_cleanups: Vec<FiberScopeCleanup>,
    pub scopes: Vec<FiberScope>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberReturnPoint {
    /// Exact caller cursor to restore after the callee returns.
    ///
    /// Static calls resolve their declared resume point to this cursor before
    /// entering the callee. Dynamic calls may resume at the instruction after
    /// the call without requiring a synthetic block or resume-point record.
    pub cursor: FiberCursor,
    pub destination: Option<AwbcRegisterId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberScope {
    pub id: AwbcScopeId,
    pub depth: u32,
    pub cleanups: Vec<FiberScopeCleanup>,
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
        content: AwbcContentUnitId,
        values: Box<[RuntimeDialogueValueBinding]>,
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

/// Exact await-handle authority retained by a suspended fiber.
///
/// Task handles keep the existing explicit task lifecycle. Need handles carry
/// only their typed identity; their Ready/Err payload is supplied through the
/// in-memory `RuntimeNeedState` step boundary rather than a runtime-value or
/// bytecode compatibility surrogate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum FiberAwaitTarget {
    Task(RuntimeValue),
    Need(NeedId),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberAwaitManyState {
    pub plan: AwbcTaskPlanId,
    pub binding: Option<AwbcPatternId>,
    pub items: Vec<RuntimeValue>,
    pub next_index: u32,
    pub in_flight: Vec<FiberAwaitManyInFlight>,
    pub results: Vec<Option<RuntimeValue>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberAwaitManyInFlight {
    pub index: u32,
    pub task_id: String,
    pub need_id: String,
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

/// Rollback snapshot used to guarantee effect-free VM fallback.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberCheckpoint {
    state: Box<FiberState>,
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
    pub entry: AwbcEntryId,
    pub cursor: FiberCursor,
    pub frames: Vec<AwbcFiberFrameSnapshot>,
    pub status: FiberStatus,
    pub suspension: Option<AwbcFiberSuspensionSnapshot>,
    pub terminal: Option<AwbcFiberTerminalSnapshot>,
    pub budget: FiberBudget,
    pub line_cursor: u64,
    pub streams: Vec<AwbcFiberStreamSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberFrameSnapshot {
    pub instance: RuntimeFrameInstanceId,
    pub function: AwbcFunctionId,
    pub layout: AwbcFrameLayoutId,
    pub return_to: Option<FiberReturnPoint>,
    pub registers: Vec<Option<AwbcRuntimeValueSnapshot>>,
    pub root_cleanups: Vec<AwbcFiberScopeCleanupSnapshot>,
    pub scopes: Vec<AwbcFiberScopeSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberScopeSnapshot {
    pub id: AwbcScopeId,
    pub depth: u32,
    pub cleanups: Vec<AwbcFiberScopeCleanupSnapshot>,
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
        content: AwbcContentUnitId,
        values: Box<[RuntimeDialogueValueBinding]>,
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
pub enum AwbcFiberAwaitTargetSnapshot {
    Task(AwbcRuntimeValueSnapshot),
    Need(NeedId),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberAwaitManySnapshot {
    pub plan: AwbcTaskPlanId,
    pub binding: Option<AwbcPatternId>,
    pub items: Vec<AwbcRuntimeValueSnapshot>,
    pub next_index: u32,
    pub in_flight: Vec<FiberAwaitManyInFlight>,
    pub results: Vec<Option<AwbcRuntimeValueSnapshot>>,
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
    Cancelled,
    Trapped(FiberTrap),
}

impl AwbcFiberStateSnapshot {
    pub fn from_live(state: &FiberState) -> AwbcSaveResult<Self> {
        Ok(Self {
            instance: state.instance,
            next_frame_instance: state.next_frame_instance,
            generation: state.generation,
            entry: state.entry,
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
            budget: state.budget,
            line_cursor: state.line_cursor,
            streams: state
                .streams
                .iter()
                .map(AwbcFiberStreamSnapshot::from_live)
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn into_live(self) -> AwbcSaveResult<FiberState> {
        Ok(FiberState {
            instance: self.instance,
            next_frame_instance: self.next_frame_instance,
            generation: self.generation,
            entry: self.entry,
            cursor: self.cursor,
            frames: self
                .frames
                .into_iter()
                .map(AwbcFiberFrameSnapshot::into_live)
                .collect::<Result<_, _>>()?,
            status: self.status,
            suspension: self
                .suspension
                .map(AwbcFiberSuspensionSnapshot::into_live)
                .transpose()?,
            terminal: self
                .terminal
                .map(AwbcFiberTerminalSnapshot::into_live)
                .transpose()?,
            budget: self.budget,
            line_cursor: self.line_cursor,
            streams: self
                .streams
                .into_iter()
                .map(AwbcFiberStreamSnapshot::into_live)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl AwbcFiberFrameSnapshot {
    fn from_live(frame: &FiberFrame) -> AwbcSaveResult<Self> {
        Ok(Self {
            instance: frame.instance,
            function: frame.function,
            layout: frame.layout,
            return_to: frame.return_to,
            registers: frame
                .registers
                .iter()
                .map(|value| {
                    value
                        .as_ref()
                        .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            root_cleanups: frame
                .root_cleanups
                .iter()
                .map(AwbcFiberScopeCleanupSnapshot::from_live)
                .collect::<Result<_, _>>()?,
            scopes: frame
                .scopes
                .iter()
                .map(AwbcFiberScopeSnapshot::from_live)
                .collect::<Result<_, _>>()?,
        })
    }

    fn into_live(self) -> AwbcSaveResult<FiberFrame> {
        Ok(FiberFrame {
            instance: self.instance,
            function: self.function,
            layout: self.layout,
            return_to: self.return_to,
            registers: self
                .registers
                .into_iter()
                .map(|value| {
                    value
                        .map(AwbcRuntimeValueSnapshot::into_runtime_value)
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            root_cleanups: self
                .root_cleanups
                .into_iter()
                .map(AwbcFiberScopeCleanupSnapshot::into_live)
                .collect::<Result<_, _>>()?,
            scopes: self
                .scopes
                .into_iter()
                .map(AwbcFiberScopeSnapshot::into_live)
                .collect::<Result<_, _>>()?,
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
        })
    }

    fn into_live(self) -> AwbcSaveResult<FiberScope> {
        Ok(FiberScope {
            id: self.id,
            depth: self.depth,
            cleanups: self
                .cleanups
                .into_iter()
                .map(AwbcFiberScopeCleanupSnapshot::into_live)
                .collect::<Result<_, _>>()?,
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

    fn into_live(self) -> AwbcSaveResult<FiberScopeCleanup> {
        Ok(FiberScopeCleanup {
            key: self.key,
            effect: self.effect,
            args: self
                .args
                .into_iter()
                .map(AwbcRuntimeValueSnapshot::into_runtime_value)
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

    fn into_live(self) -> AwbcSaveResult<FiberSuspension> {
        Ok(FiberSuspension {
            resume: self.resume,
            reason: self.reason.into_live()?,
        })
    }
}

impl AwbcFiberSuspensionReasonSnapshot {
    fn from_live(reason: &FiberSuspensionReason) -> AwbcSaveResult<Self> {
        Ok(match reason {
            FiberSuspensionReason::Dialogue {
                content,
                values,
                line_task_captures,
                result,
            } => Self::Dialogue {
                content: *content,
                values: values.clone(),
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

    fn into_live(self) -> AwbcSaveResult<FiberSuspensionReason> {
        Ok(match self {
            Self::Dialogue {
                content,
                values,
                line_task_captures,
                result,
            } => FiberSuspensionReason::Dialogue {
                content,
                values,
                line_task_captures: line_task_captures
                    .into_iter()
                    .map(AwbcRuntimeValueSnapshot::into_runtime_value)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                result,
            },
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
                target: target.into_live()?,
                binding,
                observer,
            },
            Self::AwaitMany(state) => FiberSuspensionReason::AwaitMany(state.into_live()?),
            Self::HostCall {
                call,
                args,
                destination,
            } => FiberSuspensionReason::HostCall {
                call,
                args: args
                    .into_iter()
                    .map(AwbcRuntimeValueSnapshot::into_runtime_value)
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
            FiberAwaitTarget::Task(value) => {
                Self::Task(AwbcRuntimeValueSnapshot::from_runtime_value(value)?)
            }
            FiberAwaitTarget::Need(need) => Self::Need(need.clone()),
        })
    }

    fn into_live(self) -> AwbcSaveResult<FiberAwaitTarget> {
        Ok(match self {
            Self::Task(value) => FiberAwaitTarget::Task(value.into_runtime_value()?),
            Self::Need(need) => FiberAwaitTarget::Need(need),
        })
    }
}

impl AwbcFiberAwaitManySnapshot {
    fn from_live(state: &FiberAwaitManyState) -> AwbcSaveResult<Self> {
        Ok(Self {
            plan: state.plan,
            binding: state.binding,
            items: state
                .items
                .iter()
                .map(AwbcRuntimeValueSnapshot::from_runtime_value)
                .collect::<Result<_, _>>()?,
            next_index: state.next_index,
            in_flight: state.in_flight.clone(),
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

    fn into_live(self) -> AwbcSaveResult<FiberAwaitManyState> {
        Ok(FiberAwaitManyState {
            plan: self.plan,
            binding: self.binding,
            items: self
                .items
                .into_iter()
                .map(AwbcRuntimeValueSnapshot::into_runtime_value)
                .collect::<Result<_, _>>()?,
            next_index: self.next_index,
            in_flight: self.in_flight,
            results: self
                .results
                .into_iter()
                .map(|value| {
                    value
                        .map(AwbcRuntimeValueSnapshot::into_runtime_value)
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

    fn into_live(self) -> AwbcSaveResult<FiberStreamState> {
        Ok(FiberStreamState {
            plan: self.plan,
            queue: self
                .queue
                .into_iter()
                .map(AwbcRuntimeValueSnapshot::into_runtime_value)
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
            FiberTerminalValue::Cancelled => Self::Cancelled,
            FiberTerminalValue::Trapped(trap) => Self::Trapped(trap.clone()),
        })
    }

    fn into_live(self) -> AwbcSaveResult<FiberTerminalValue> {
        Ok(match self {
            Self::Returned(value) => FiberTerminalValue::Returned(
                value
                    .map(AwbcRuntimeValueSnapshot::into_runtime_value)
                    .transpose()?,
            ),
            Self::Cancelled => FiberTerminalValue::Cancelled,
            Self::Trapped(trap) => FiberTerminalValue::Trapped(trap),
        })
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FiberStateError {
    #[error(transparent)]
    RuntimeIdentity(#[from] crate::runtime_id::RuntimeIdExhausted),
    #[error("AWBC entry {0} does not exist")]
    UnknownEntry(u32),
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
    #[error("fiber frame function/layout pair is invalid")]
    InvalidFrame,
    #[error("fiber call return value does not match its destination")]
    ReturnValueMismatch,
    #[error("structured expression function bodies are not valid in an AWBC fiber")]
    StructuredRuntimeFunction,
    #[error("invalid AWBC runtime function: {reason}")]
    InvalidRuntimeFunction { reason: String },
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

impl FiberState {
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
            entry,
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
        Self::for_function(program, entry, function, generation, budget_quantum)
    }

    /// Creates an internal function fiber. Entry target membership is not an
    /// invariant of trait methods, stream transforms, pure calls, or child
    /// functions; external entry/route selection must use
    /// [`Self::for_entry_target_function`].
    pub fn for_function(
        program: &AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        generation: u64,
        budget_quantum: u64,
    ) -> Result<Self, FiberStateError> {
        Self::for_function_with_instance(
            program,
            entry,
            function,
            RuntimeFiberInstanceId::from_allocated(std::num::NonZeroU64::MIN),
            generation,
            budget_quantum,
        )
    }

    pub(crate) fn for_function_with_instance(
        program: &AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        instance: RuntimeFiberInstanceId,
        generation: u64,
        budget_quantum: u64,
    ) -> Result<Self, FiberStateError> {
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
            entry,
            cursor: FiberCursor {
                function,
                block: function_record.entry_block,
                instruction_offset: 0,
            },
            frames: vec![frame],
            status: FiberStatus::Running,
            suspension: None,
            terminal: None,
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

    /// Atomically removes the root function's current parameter values in
    /// sealed positional order. Missing values represent parameters consumed
    /// by the child and are preserved as such for the custody reducer.
    pub(crate) fn take_function_argument_values(
        &mut self,
        program: &AwbcProgram,
    ) -> Result<Vec<Option<RuntimeValue>>, FiberStateError> {
        self.active_frame_mut()?.take_positional_arguments(program)
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

        let mut register_values = frame.registers.clone();
        for (position, value, name) in assignments {
            let (register, slot) = parameters[position];
            let expected = signature.params[position];
            if slot.ty != expected {
                return Err(FiberStateError::InvalidFrame);
            }
            if !runtime_value_matches_type(program, value, expected, 0) {
                return Err(FiberStateError::ArgumentType {
                    name: name.to_owned(),
                    expected: runtime_type_label(program, expected),
                    actual: runtime_value_type_label(value),
                });
            }
            register_values[register] = Some(value.clone());
        }
        self.active_frame_mut()?.registers = register_values;
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
            register_values[register] = Some(binding.value.clone());
        }
        self.active_frame_mut()?.registers = register_values;
        Ok(())
    }

    pub fn checkpoint(&self) -> FiberCheckpoint {
        FiberCheckpoint {
            state: Box::new(self.clone()),
        }
    }

    pub fn restore(&mut self, checkpoint: FiberCheckpoint) {
        *self = *checkpoint.state;
    }

    pub fn validate_for_program(&self, program: &AwbcProgram) -> Result<(), FiberStateError> {
        if program.entries.get(self.entry.index()).is_none()
            && !(program.entries.is_empty() && self.entry == AwbcEntryId(0))
        {
            return Err(FiberStateError::UnknownEntry(self.entry.0));
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
            if index == 0 && frame.return_to.is_some() {
                return Err(FiberStateError::InvalidFrame);
            }
            if let Some(return_to) = frame.return_to {
                let caller = self
                    .frames
                    .get(index.saturating_sub(1))
                    .ok_or(FiberStateError::InvalidFrame)?;
                validate_return_point(program, caller, return_to)?;
            }
        }
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
            validate_terminal(program, terminal)?;
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
        self.cursor.instruction_offset = next_offset;
        Ok(())
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
        self.cursor = FiberCursor {
            function: point.function,
            block: point.block,
            instruction_offset: 0,
        };
        self.status = FiberStatus::Running;
        self.suspension = None;
        Ok(())
    }

    /// Selects the verified Progress continuation retained by an Await
    /// suspension, then resumes through the ordinary declared-point path.
    pub fn resume_await_observer_at(
        &mut self,
        program: &AwbcProgram,
        resume: AwbcResumePointId,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Suspended)?;
        let admitted = self.suspension.as_ref().is_some_and(|suspension| {
            matches!(
                &suspension.reason,
                FiberSuspensionReason::Await {
                    observer: Some(observer),
                    ..
                } if observer.resume == resume
            )
        });
        if !admitted {
            return Err(FiberStateError::InvalidFrame);
        }
        if let Some(suspension) = self.suspension.as_mut() {
            suspension.resume = FiberResumeTarget::Declared(resume);
        }
        self.resume_at(program, resume)
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
            FiberReturnPoint {
                cursor: FiberCursor {
                    function: point.function,
                    block: point.block,
                    instruction_offset: 0,
                },
                destination,
            },
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
        validate_return_point(program, self.active_frame()?, return_to)?;
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
        let return_to = self.active_frame()?.return_to;
        let mut next_frame_instance = self.next_frame_instance;
        let frame_instance = RuntimeFrameInstanceId::from_allocated(
            next_frame_instance.take_next(RuntimeIdNamespace::FrameInstance)?,
        );
        let mut frame = FiberFrame::new(frame_instance, program, function, return_to)?;
        frame.bind_positional_arguments(program, args)?;
        self.next_frame_instance = next_frame_instance;
        *self.active_frame_mut()? = frame;
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
        let Some(return_to) = frame.return_to else {
            return Ok(None);
        };
        let caller = self
            .frames
            .get(self.frames.len().saturating_sub(2))
            .ok_or(FiberStateError::MissingFrame)?;
        validate_return_point(program, caller, return_to)?;
        self.frames.pop();
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
        self.require_status(FiberStatus::Running)?;
        if self.frames.len() == 1 {
            self.mark_returned(value)?;
            return Ok(true);
        }
        let returning_frame = self.frames.last().ok_or(FiberStateError::MissingFrame)?;
        let return_to = returning_frame
            .return_to
            .ok_or(FiberStateError::InvalidFrame)?;
        let signature = program
            .functions
            .get(returning_frame.function.index())
            .and_then(|function| program.signatures.get(function.signature.index()))
            .ok_or(FiberStateError::InvalidFrame)?;
        let return_value = match (signature.result, value) {
            (Some(expected), Some(value))
                if runtime_value_matches_type(program, &value, expected, 0) =>
            {
                Some(value)
            }
            (None, None) if return_to.destination.is_some() => Some(RuntimeValue::Unit),
            (None, None) => None,
            _ => return Err(FiberStateError::ReturnValueMismatch),
        };
        if let (Some(destination), Some(value)) = (return_to.destination, return_value.as_ref()) {
            let caller_frame = self
                .frames
                .get(self.frames.len() - 2)
                .ok_or(FiberStateError::MissingFrame)?;
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
            if !runtime_value_matches_type(program, value, destination_type, 0) {
                return Err(FiberStateError::ReturnValueMismatch);
            }
        }
        let popped = self
            .pop_call_frame(program)?
            .ok_or(FiberStateError::InvalidFrame)?;
        debug_assert_eq!(popped, return_to);
        if let (Some(destination), Some(value)) = (return_to.destination, return_value) {
            self.active_frame_mut()?.set_register(destination, value)?;
        }
        Ok(false)
    }

    pub fn mark_returned(&mut self, value: Option<RuntimeValue>) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        self.status = FiberStatus::Returned;
        self.suspension = None;
        self.terminal = Some(FiberTerminalValue::Returned(value));
        Ok(())
    }

    pub(super) fn mark_cancelled(&mut self) {
        self.status = FiberStatus::Cancelled;
        self.suspension = None;
        self.terminal = Some(FiberTerminalValue::Cancelled);
    }

    pub fn mark_trapped(&mut self, trap: FiberTrap) {
        if matches!(
            self.status,
            FiberStatus::Returned | FiberStatus::Cancelled | FiberStatus::Trapped
        ) {
            return;
        }
        self.status = FiberStatus::Trapped;
        self.suspension = None;
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
            if state.suspension.is_some() || state.terminal.is_some() {
                return Err(FiberStateError::InvalidStatus {
                    actual: state.status,
                    expected: FiberStatus::Running,
                });
            }
        }
        FiberStatus::Suspended => {
            if state.suspension.is_none() || state.terminal.is_some() {
                return Err(FiberStateError::InvalidStatus {
                    actual: state.status,
                    expected: FiberStatus::Suspended,
                });
            }
        }
        FiberStatus::Returned => {
            if state.suspension.is_some()
                || !matches!(
                    state.terminal.as_ref(),
                    Some(FiberTerminalValue::Returned(_))
                )
            {
                return Err(FiberStateError::InvalidStatus {
                    actual: state.status,
                    expected: state.status,
                });
            }
        }
        FiberStatus::Cancelled => {
            if state.suspension.is_some()
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
    let layout = program
        .frame_layouts
        .get(frame.layout.index())
        .ok_or(FiberStateError::UnknownFrameLayout(frame.layout.0))?;
    if frame.registers.len() != layout.slots.len() {
        return Err(FiberStateError::InvalidFrame);
    }
    for (index, value) in frame.registers.iter().enumerate() {
        let Some(value) = value else {
            continue;
        };
        let slot = layout
            .slots
            .get(index)
            .ok_or(FiberStateError::InvalidFrame)?;
        validate_runtime_value_at(
            program,
            value,
            Some(slot.ty),
            format!("{path}.registers[{index}]"),
        )?;
    }
    for (index, cleanup) in frame.root_cleanups.iter().enumerate() {
        validate_cleanup(program, cleanup, &format!("{path}.root_cleanups[{index}]"))?;
    }
    for (scope_index, scope) in frame.scopes.iter().enumerate() {
        if scope.depth > layout.max_scope_depth {
            return Err(FiberStateError::InvalidFrame);
        }
        for (cleanup_index, cleanup) in scope.cleanups.iter().enumerate() {
            validate_cleanup(
                program,
                cleanup,
                &format!("{path}.scopes[{scope_index}].cleanups[{cleanup_index}]"),
            )?;
        }
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
        return Err(FiberStateError::InvalidRuntimeFunction {
            reason: format!(
                "runtime value nesting exceeds {} levels",
                crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH
            ),
        });
    }
    match value {
        RuntimeValue::Function(function) => validate_runtime_function(program, function, depth),
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
                return Err(FiberStateError::InvalidRuntimeFunction {
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
        | RuntimeValue::Char(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::Progress(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(RuntimeIterator::Range(_))
        | RuntimeValue::EntityRef(_)
        | RuntimeValue::Variant { payload: None, .. } => Ok(()),
    }
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

fn validate_runtime_function(
    program: &AwbcProgram,
    function: &RuntimeFunctionValue,
    depth: usize,
) -> Result<(), FiberStateError> {
    let RuntimeFunctionBody::Awbc(closure) = function.body() else {
        return Err(FiberStateError::StructuredRuntimeFunction);
    };
    let function_id = closure.function();
    let function_record = program
        .functions
        .get(function_id.index())
        .ok_or(FiberStateError::UnknownFunction(function_id.0))?;
    let signature = program
        .signatures
        .get(function_record.signature.index())
        .ok_or_else(|| FiberStateError::InvalidRuntimeFunction {
            reason: format!("function {} has no signature", function_id.0),
        })?;
    let layout = program
        .frame_layouts
        .get(function_record.frame_layout.index())
        .ok_or(FiberStateError::UnknownFrameLayout(
            function_record.frame_layout.0,
        ))?;
    let parameters = layout
        .slots
        .iter()
        .filter(|slot| slot.role == AwbcFrameSlotRole::Parameter)
        .collect::<Vec<_>>();
    let stored_arity = closure
        .captures()
        .len()
        .saturating_add(closure.remaining_params().len());
    if signature.params.len() != stored_arity || parameters.len() != stored_arity {
        return Err(FiberStateError::InvalidRuntimeFunction {
            reason: format!(
                "function {} expects {} parameters, snapshot stores {} captures/parameters",
                function_id.0,
                signature.params.len(),
                stored_arity
            ),
        });
    }

    let stored_names = closure
        .captures()
        .iter()
        .map(|capture| capture.name.as_str())
        .chain(closure.remaining_params().iter().map(String::as_str))
        .collect::<Vec<_>>();
    let mut unique_names = BTreeSet::new();
    for (position, (name, slot)) in stored_names.iter().zip(&parameters).enumerate() {
        if name.is_empty() || !unique_names.insert(*name) {
            return Err(FiberStateError::InvalidRuntimeFunction {
                reason: format!(
                    "function {} has an empty or duplicate binding name at position {position}",
                    function_id.0
                ),
            });
        }
        let expected_name = slot
            .name
            .and_then(|name| program.strings.get(name.index()))
            .ok_or_else(|| FiberStateError::InvalidRuntimeFunction {
                reason: format!(
                    "function {} parameter {position} has no stable name",
                    function_id.0
                ),
            })?;
        if *name != expected_name {
            return Err(FiberStateError::InvalidRuntimeFunction {
                reason: format!(
                    "function {} binding {position} is `{name}`, expected `{expected_name}`",
                    function_id.0
                ),
            });
        }
        if slot.ty != signature.params[position] {
            return Err(FiberStateError::InvalidRuntimeFunction {
                reason: format!(
                    "function {} parameter {position} disagrees with its signature",
                    function_id.0
                ),
            });
        }
    }
    for (position, capture) in closure.captures().iter().enumerate() {
        if !runtime_value_matches_type(program, &capture.value, signature.params[position], 0) {
            return Err(FiberStateError::InvalidRuntimeFunction {
                reason: format!(
                    "function {} capture `{}` has type {}, expected {}",
                    function_id.0,
                    capture.name,
                    runtime_value_type_label(&capture.value),
                    runtime_type_label(program, signature.params[position])
                ),
            });
        }
        validate_nested_runtime_value(program, &capture.value, depth + 1)?;
    }
    Ok(())
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
    return_to: FiberReturnPoint,
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
    Ok(())
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
            content,
            values,
            line_task_captures,
            result,
        } => {
            let Some(content) = program.content_units.get(content.index()) else {
                return Err(FiberStateError::InvalidFrame);
            };
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
            validate_await_suspension(program, target, *binding)?;
            if observer.is_some_and(|observer| {
                observer.destination.index() >= frame.registers.len()
                    || program.resume_points.get(observer.resume.index()).is_none()
            }) {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        FiberSuspensionReason::AwaitMany(await_many) => {
            validate_await_many_suspension(program, await_many)?;
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

fn validate_await_suspension(
    program: &AwbcProgram,
    target: &FiberAwaitTarget,
    binding: Option<AwbcPatternId>,
) -> Result<(), FiberStateError> {
    if binding.is_some_and(|binding| program.patterns.get(binding.index()).is_none()) {
        return Err(FiberStateError::InvalidFrame);
    }
    match target {
        FiberAwaitTarget::Task(task) if !matches!(task, RuntimeValue::String(_)) => {
            return Err(FiberStateError::InvalidRuntimeValue {
                path: "suspension.await.target".to_owned(),
                reason: format!(
                    "expected task handle, received {}",
                    runtime_value_type_label(task)
                ),
            });
        }
        FiberAwaitTarget::Need(need) if need.0.is_empty() => {
            return Err(FiberStateError::InvalidRuntimeValue {
                path: "suspension.await.target".to_owned(),
                reason: "Need identity must not be empty".to_owned(),
            });
        }
        FiberAwaitTarget::Task(_) | FiberAwaitTarget::Need(_) => {}
    }
    Ok(())
}

fn validate_await_many_suspension(
    program: &AwbcProgram,
    await_many: &FiberAwaitManyState,
) -> Result<(), FiberStateError> {
    let plan = program
        .task_plans
        .get(await_many.plan.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let signature = program
        .signatures
        .get(plan.signature.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    if plan.many.is_none()
        || plan.arguments.len() != signature.params.len()
        || await_many
            .binding
            .is_some_and(|binding| program.patterns.get(binding.index()).is_none())
        || await_many.results.len() > await_many.items.len()
        || await_many.next_index as usize > await_many.items.len()
        || await_many
            .in_flight
            .iter()
            .any(|in_flight| in_flight.index as usize >= await_many.items.len())
    {
        return Err(FiberStateError::InvalidFrame);
    }
    let item_type = match signature.params.as_slice() {
        [] => None,
        [item] => Some(*item),
        _ => return Err(FiberStateError::InvalidFrame),
    };
    for (index, item) in await_many.items.iter().enumerate() {
        validate_runtime_value_at(
            program,
            item,
            item_type,
            format!("suspension.await_many.items[{index}]"),
        )?;
    }
    for (index, result) in await_many.results.iter().enumerate() {
        if let Some(result) = result {
            validate_runtime_value_at(
                program,
                result,
                signature.result,
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
    }
    match (signature.result, destination) {
        (None, None) => Ok(()),
        (Some(_), Some(destination)) if destination.index() < frame.registers.len() => Ok(()),
        _ => Err(FiberStateError::InvalidFrame),
    }
}

fn validate_terminal(
    program: &AwbcProgram,
    terminal: &FiberTerminalValue,
) -> Result<(), FiberStateError> {
    match terminal {
        FiberTerminalValue::Returned(Some(value)) => {
            validate_runtime_value_at(program, value, None, "terminal.returned".to_owned())
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
            layout: function_record.frame_layout,
            return_to,
            registers: vec![None; layout.slots.len()],
            root_cleanups: Vec::new(),
            scopes: Vec::with_capacity(layout.max_scope_depth as usize),
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
        let mut next = self.registers.clone();
        for (position, ((register, slot), value)) in parameters.iter().zip(args).enumerate() {
            let expected = signature.params[position];
            if slot.ty != expected || !runtime_value_matches_type(program, value, expected, 0) {
                return Err(FiberStateError::ArgumentType {
                    name: slot
                        .name
                        .and_then(|id| program.strings.get(id.index()).cloned())
                        .unwrap_or_else(|| format!("${position}")),
                    expected: runtime_type_label(program, expected),
                    actual: runtime_value_type_label(value),
                });
            }
            next[*register] = Some(value.clone());
        }
        self.registers = next;
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
        for (position, ((_, slot), value)) in parameters.iter().zip(&args).enumerate() {
            let expected = signature.params[position];
            if slot.ty != expected || !runtime_value_matches_type(program, value, expected, 0) {
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
        let mut next = self.registers.clone();
        for ((register, _), value) in parameters.into_iter().zip(args) {
            next[register] = Some(value);
        }
        self.registers = next;
        Ok(())
    }

    pub(crate) fn take_positional_arguments(
        &mut self,
        program: &AwbcProgram,
    ) -> Result<Vec<Option<RuntimeValue>>, FiberStateError> {
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
            .map(|(register, _)| register)
            .collect::<Vec<_>>();
        if parameters.len() != signature.params.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        let mut next = self.registers.clone();
        let values = parameters
            .into_iter()
            .map(|register| next[register].take())
            .collect();
        self.registers = next;
        Ok(values)
    }

    pub fn register(&self, register: AwbcRegisterId) -> Result<&RuntimeValue, FiberStateError> {
        self.registers
            .get(register.index())
            .and_then(Option::as_ref)
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
        *slot = Some(value);
        Ok(())
    }

    pub fn clear_register(&mut self, register: AwbcRegisterId) -> Result<(), FiberStateError> {
        let slot = self.registers.get_mut(register.index()).ok_or(
            FiberStateError::RegisterOutOfBounds {
                register: register.0,
                layout: self.layout.0,
            },
        )?;
        *slot = None;
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

#[allow(
    clippy::too_many_lines,
    reason = "runtime value admission exhaustively mirrors the closed AWBC runtime-type family"
)]
pub(crate) fn runtime_value_matches_type(
    program: &AwbcProgram,
    value: &RuntimeValue,
    ty: AwbcTypeId,
    depth: usize,
) -> bool {
    if depth > 64 {
        return false;
    }
    let type_id = ty;
    let Some(ty) = program.runtime_types.get(type_id.index()) else {
        return false;
    };
    match (value, ty.shape()) {
        (RuntimeValue::Reduction(value), AwbcRuntimeTypeShape::Opaque { arguments, .. }) => program
            .opaque_owner(type_id)
            .ok()
            .flatten()
            .is_some_and(|owner| {
                owner == *value.owner()
                    && arguments.len() == 1
                    && runtime_value_matches_type(program, value.state(), arguments[0], depth + 1)
            }),
        (_, AwbcRuntimeTypeShape::Dynamic)
        | (
            RuntimeValue::String(_),
            AwbcRuntimeTypeShape::String
                | AwbcRuntimeTypeShape::Task(_)
                | AwbcRuntimeTypeShape::Need(_),
        )
        | (RuntimeValue::Unit, AwbcRuntimeTypeShape::Unit)
        | (RuntimeValue::Bool(_), AwbcRuntimeTypeShape::Bool)
        | (RuntimeValue::F32(_), AwbcRuntimeTypeShape::F32)
        | (RuntimeValue::F64(_), AwbcRuntimeTypeShape::F64)
        | (RuntimeValue::Char(_), AwbcRuntimeTypeShape::Char)
        | (RuntimeValue::Duration(_), AwbcRuntimeTypeShape::Duration)
        | (RuntimeValue::Progress(_), AwbcRuntimeTypeShape::Progress)
        | (RuntimeValue::EntityRef(_), AwbcRuntimeTypeShape::EntityRef)
        | (RuntimeValue::MatrixF32(_), AwbcRuntimeTypeShape::MatrixF32)
        | (RuntimeValue::MatrixF64(_), AwbcRuntimeTypeShape::MatrixF64)
        | (RuntimeValue::TensorF32(_), AwbcRuntimeTypeShape::TensorF32)
        | (RuntimeValue::TensorF64(_), AwbcRuntimeTypeShape::TensorF64) => true,
        (RuntimeValue::Agent(value), AwbcRuntimeTypeShape::Agent(expected)) => {
            value.operational_type() == expected.operational_type()
        }
        (RuntimeValue::Record(_), AwbcRuntimeTypeShape::Agent(expected)) => {
            expected.operational_type().accepts_protocol_record()
        }
        (RuntimeValue::Seq(values), AwbcRuntimeTypeShape::Bytes) => values
            .clone()
            .into_values()
            .iter()
            .all(|value| matches!(value, RuntimeValue::UInt(value) if value.width() == crate::value::RuntimeUnsignedIntWidth::U8)),
        (RuntimeValue::Int(value), AwbcRuntimeTypeShape::Int(kind)) => signed_kind(*value) == *kind,
        (RuntimeValue::UInt(value), AwbcRuntimeTypeShape::UInt(kind)) => unsigned_kind(*value) == *kind,
        (RuntimeValue::Opaque(value), AwbcRuntimeTypeShape::Opaque { .. }) => program
            .opaque_owner(type_id)
            .ok()
            .flatten()
            .is_some_and(|owner| owner.accepts_opaque_value(value)),
        (RuntimeValue::Tuple(values), AwbcRuntimeTypeShape::Tuple(types)) => {
            values.len() == types.len()
                && values
                    .iter()
                    .zip(types)
                    .all(|(value, ty)| runtime_value_matches_type(program, value, *ty, depth + 1))
        }
        (RuntimeValue::Seq(values), AwbcRuntimeTypeShape::Sequence(item)) => values
            .clone()
            .into_values()
            .iter()
            .all(|value| runtime_value_matches_type(program, value, *item, depth + 1)),
        (RuntimeValue::Seq(values), AwbcRuntimeTypeShape::Array { item, length }) => {
            values.len() == usize::try_from(*length).unwrap_or(usize::MAX)
                && values
                    .clone()
                    .into_values()
                    .iter()
                    .all(|value| runtime_value_matches_type(program, value, *item, depth + 1))
        }
        (RuntimeValue::Record(values), AwbcRuntimeTypeShape::Record { fields, .. }) => {
            values.len() == fields.len()
                && values.iter().zip(fields).all(|(value, field)| {
                    runtime_value_matches_type(program, value.value(), field.ty, depth + 1)
                })
        }
        (
            RuntimeValue::Variant {
                owner: actual_owner,
                ordinal,
                name,
                payload,
            },
            AwbcRuntimeTypeShape::Variant { owner, cases, .. },
        ) => {
            runtime_variant_identity(program, ty.semantic_identity(), owner).as_ref()
                == Some(actual_owner)
                && usize::try_from(*ordinal)
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .is_some_and(|case| {
                        program
                            .strings
                            .get(case.name.index())
                            .is_some_and(|case_name| {
                                case_name == name
                                    && match (case.payload, payload.as_deref()) {
                                        (None, None) => true,
                                        (Some(ty), Some(value)) => runtime_value_matches_type(
                                            program,
                                            value,
                                            ty,
                                            depth + 1,
                                        ),
                                        _ => false,
                                    }
                            })
                    })
        }
        (value, AwbcRuntimeTypeShape::Choice(alternatives)) => alternatives
            .iter()
            .any(|alternative| runtime_value_matches_type(program, value, *alternative, depth + 1)),
        (
            RuntimeValue::NominalRecord(record),
            AwbcRuntimeTypeShape::Nominal {
                public_id, layout, ..
            },
        ) => {
            program
                .strings
                .get(public_id.index())
                .is_some_and(|expected| record.type_id().as_str() == expected)
                && record.layout().as_bytes() == layout
        }
        (
            RuntimeValue::NominalRecord(record),
            AwbcRuntimeTypeShape::NominalRecord { .. },
        ) => program
            .nominal_record_layout(type_id)
            .ok()
            .flatten()
            .is_some_and(|layout| record.validate_against_layout(&layout).is_ok()),
        _ => false,
    }
}

pub(crate) fn runtime_variant_identity(
    program: &AwbcProgram,
    semantic_identity: RuntimeSemanticTypeId,
    owner: &AwbcVariantIdentity,
) -> Option<RuntimeVariantIdentity> {
    match owner {
        AwbcVariantIdentity::Nominal { public_id } => Some(RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new(program.strings.get(public_id.index())?.clone())
                .ok()?,
            semantic_identity,
        }),
        AwbcVariantIdentity::Builtin(owner) => Some(RuntimeVariantIdentity::Builtin(*owner)),
    }
}

fn signed_kind(value: RuntimeInt) -> AwbcSignedIntKind {
    match value {
        RuntimeInt::I8(_) => AwbcSignedIntKind::I8,
        RuntimeInt::I16(_) => AwbcSignedIntKind::I16,
        RuntimeInt::I32(_) => AwbcSignedIntKind::I32,
        RuntimeInt::I64(_) => AwbcSignedIntKind::I64,
        RuntimeInt::I128(_) => AwbcSignedIntKind::I128,
        RuntimeInt::ISize(_) => AwbcSignedIntKind::ISize,
    }
}

fn unsigned_kind(value: RuntimeUInt) -> AwbcUnsignedIntKind {
    match value {
        RuntimeUInt::U8(_) => AwbcUnsignedIntKind::U8,
        RuntimeUInt::U16(_) => AwbcUnsignedIntKind::U16,
        RuntimeUInt::U32(_) => AwbcUnsignedIntKind::U32,
        RuntimeUInt::U64(_) => AwbcUnsignedIntKind::U64,
        RuntimeUInt::U128(_) => AwbcUnsignedIntKind::U128,
        RuntimeUInt::USize(_) => AwbcUnsignedIntKind::USize,
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
        RuntimeValue::Function(_) => "function",
        RuntimeValue::Variant { .. } => "variant",
    }
    .to_owned()
}

#[cfg(test)]
#[allow(clippy::default_trait_access, clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::awbc::schema::{
        AwbcBlock, AwbcEntry, AwbcEntryKind, AwbcFlowBinding, AwbcFrameLayout, AwbcFrameSlot,
        AwbcFrameSlotRole, AwbcFunction, AwbcFunctionFlags, AwbcFunctionKind, AwbcRuntimeType,
        AwbcSafePointKind, AwbcSignature, AwbcStringId, AwbcTableRange, AwbcTerminator,
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
            slots: Vec::new(),
            max_scope_depth: 0,
        });
        program.functions.push(AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: Default::default(),
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
        program.frame_layouts[0].slots.push(AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        });
        program
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
            Err(FiberStateError::InvalidRuntimeFunction { reason })
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
            vec![Some(RuntimeValue::Unit)]
        );
    }

    #[test]
    fn bind_function_argument_values_rejects_arity_and_type_without_mutation() {
        let program = one_unit_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        let before = fiber.clone();

        assert_eq!(
            fiber.bind_function_argument_values(&program, &[]),
            Err(FiberStateError::ArgumentCount {
                expected: 1,
                actual: 0,
            })
        );
        assert_eq!(fiber, before);

        assert!(matches!(
            fiber.bind_function_argument_values(&program, &[RuntimeValue::Bool(true)]),
            Err(FiberStateError::ArgumentType { .. })
        ));
        assert_eq!(fiber, before);
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
        let before = fiber.clone();

        assert!(matches!(
            fiber.commit_yielded_instruction(stale),
            Err(FiberStateError::StaleCursor { .. })
        ));
        assert_eq!(fiber, before);
    }

    #[test]
    fn commit_yielded_instruction_rejects_offset_overflow_without_mutation() {
        let program = zero_parameter_entry_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        fiber.cursor.instruction_offset = u32::MAX;
        let observed = fiber.cursor;
        let before = fiber.clone();

        assert!(matches!(
            fiber.commit_yielded_instruction(observed),
            Err(FiberStateError::InstructionOffsetOverflow { cursor }) if cursor == observed
        ));
        assert_eq!(fiber, before);
    }
}
