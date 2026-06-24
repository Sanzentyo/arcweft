use crate::artifact::CodeRegionId;
use crate::policy::ProgramGenerationId;
use arcweft_core::awbc::fiber::{
    FiberSafePoint, FiberState, FiberStateError, FiberStatus, FiberSuspension,
    FiberSuspensionReason, FiberTrap,
};
use arcweft_core::awbc::schema::{
    AwbcBlockId, AwbcDigest, AwbcFunctionId, AwbcHostCallId, AwbcOpcode, AwbcProgram,
    AwbcRegisterId, AwbcResumePointId, AwbcSafePointKind, AwbcSourceMapId, AwbcTrapCode,
    AwbcTypeId,
};
use arcweft_core::value::RuntimeValue;
use std::fmt;

pub const COMPILED_REGION_ABI_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledExecutionIdentity {
    pub generation: ProgramGenerationId,
    pub program_digest: AwbcDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRegionMetadata {
    pub abi_version: u32,
    pub region: CodeRegionId,
    pub program_digest: AwbcDigest,
    pub runtime_layout_digest: AwbcDigest,
    pub host_abi_digest: AwbcDigest,
    pub entries: Vec<CompiledRegionEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledRegionEntry {
    pub function: AwbcFunctionId,
    pub block: AwbcBlockId,
    pub resume: Option<AwbcResumePointId>,
}

/// Safe Rust baseline ABI. A compiled region receives no host interface, so it
/// cannot perform external I/O; host operations are returned as structured exits.
pub trait CompiledRegion: Send + Sync {
    fn metadata(&self) -> &CompiledRegionMetadata;

    fn step(&self, input: CompiledRegionInput<'_>) -> CompiledRegionResult;
}

pub struct CompiledRegionInput<'a> {
    pub program: &'a AwbcProgram,
    pub fiber: &'a mut FiberState,
    pub instruction_budget: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledRegionResult {
    pub consumed: u64,
    pub exit: CompiledStepExit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompiledStepExit {
    Continue { next: FiberSafePoint },
    HostRequest(CompiledHostRequest),
    Suspended(FiberSuspension),
    Returned(Option<RuntimeValue>),
    BudgetExhausted { resume: AwbcResumePointId },
    Failed(RuntimeFailure),
    FallbackToVm(CompiledFallback),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledHostRequest {
    pub call: AwbcHostCallId,
    pub args: Vec<RuntimeValue>,
    pub destination: Option<AwbcRegisterId>,
    pub resume: AwbcResumePointId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFailure {
    pub code: AwbcTrapCode,
    pub message: Option<String>,
    pub source_map: Option<AwbcSourceMapId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledFallback {
    pub reason: CompiledFallbackReason,
    pub at: FiberSafePoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompiledFallbackReason {
    UnsupportedOpcode(AwbcOpcode),
    UnsupportedType(AwbcTypeId),
    UnsupportedIntrinsic(u32),
    DynamicTarget,
    RegionNotFound,
    StaleGeneration,
    ArtifactRejected,
    HostAbiMismatch,
    RuntimeLayoutMismatch,
    BackendUnavailable,
    BudgetPreemption,
    ExplicitDevSelection,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompiledTransition {
    Continue,
    HostRequest(CompiledHostRequest),
    Suspended,
    Returned(Option<RuntimeValue>),
    BudgetExhausted,
    Failed(RuntimeFailure),
    FallbackToVm(CompiledFallback),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompiledApplyError {
    AbiVersion { actual: u32, expected: u32 },
    ProgramDigest,
    RuntimeLayoutDigest,
    HostAbiDigest,
    Generation { fiber: u64, expected: u64 },
    InvalidEntry,
    InvalidSafePoint,
    BudgetContract { consumed: u64, available: u64 },
    FallbackConsumedBudget { consumed: u64 },
    Fiber(FiberStateError),
}

impl fmt::Display for CompiledApplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiVersion { actual, expected } => {
                write!(formatter, "compiled ABI {actual} does not match {expected}")
            }
            Self::ProgramDigest => formatter.write_str("compiled program digest mismatch"),
            Self::RuntimeLayoutDigest => {
                formatter.write_str("compiled runtime-layout digest mismatch")
            }
            Self::HostAbiDigest => formatter.write_str("compiled host-ABI digest mismatch"),
            Self::Generation { fiber, expected } => write!(
                formatter,
                "compiled generation mismatch: fiber {fiber}, expected {expected}"
            ),
            Self::InvalidEntry => formatter.write_str("fiber is not at a compiled-region entry"),
            Self::InvalidSafePoint => {
                formatter.write_str("compiled exit is not a valid safe point")
            }
            Self::BudgetContract {
                consumed,
                available,
            } => write!(
                formatter,
                "compiled region consumed {consumed} with only {available} available"
            ),
            Self::FallbackConsumedBudget { consumed } => write!(
                formatter,
                "compiled VM fallback consumed {consumed}; fallback must consume zero"
            ),
            Self::Fiber(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompiledApplyError {}

impl From<FiberStateError> for CompiledApplyError {
    fn from(value: FiberStateError) -> Self {
        Self::Fiber(value)
    }
}

/// Executes one region transactionally. Fallback and failure restore the entry
/// checkpoint before handing control to the VM or materializing a trap.
pub fn execute_compiled_region<R: CompiledRegion + ?Sized>(
    region: &R,
    identity: CompiledExecutionIdentity,
    program: &AwbcProgram,
    fiber: &mut FiberState,
    instruction_budget: u64,
) -> Result<CompiledTransition, CompiledApplyError> {
    validate_metadata(region.metadata(), identity, program, fiber)?;
    let available = instruction_budget.min(fiber.budget.remaining);
    let entry_budget = fiber.budget;
    let checkpoint = fiber.checkpoint();
    let result = region.step(CompiledRegionInput {
        program,
        fiber,
        instruction_budget: available,
    });
    if result.consumed > available {
        fiber.restore(checkpoint);
        return Err(CompiledApplyError::BudgetContract {
            consumed: result.consumed,
            available,
        });
    }
    if matches!(result.exit, CompiledStepExit::FallbackToVm(_)) {
        fiber.restore(checkpoint);
        if result.consumed != 0 {
            return Err(CompiledApplyError::FallbackConsumedBudget {
                consumed: result.consumed,
            });
        }
        return apply_exit(program, fiber, result.exit);
    }
    if matches!(result.exit, CompiledStepExit::Failed(_)) {
        fiber.restore(checkpoint.clone());
    } else {
        // Budget accounting belongs to the dispatcher, not generated code.
        fiber.budget = entry_budget;
    }
    if !fiber.consume_budget(result.consumed) {
        fiber.restore(checkpoint);
        return Err(CompiledApplyError::BudgetContract {
            consumed: result.consumed,
            available,
        });
    }
    apply_exit(program, fiber, result.exit)
}

fn validate_metadata(
    metadata: &CompiledRegionMetadata,
    identity: CompiledExecutionIdentity,
    program: &AwbcProgram,
    fiber: &FiberState,
) -> Result<(), CompiledApplyError> {
    if metadata.abi_version != COMPILED_REGION_ABI_VERSION {
        return Err(CompiledApplyError::AbiVersion {
            actual: metadata.abi_version,
            expected: COMPILED_REGION_ABI_VERSION,
        });
    }
    if metadata.program_digest != identity.program_digest {
        return Err(CompiledApplyError::ProgramDigest);
    }
    if metadata.runtime_layout_digest != program.header.runtime_layout_digest {
        return Err(CompiledApplyError::RuntimeLayoutDigest);
    }
    if metadata.host_abi_digest != program.header.host_abi_digest {
        return Err(CompiledApplyError::HostAbiDigest);
    }
    if fiber.generation != identity.generation.0 {
        return Err(CompiledApplyError::Generation {
            fiber: fiber.generation,
            expected: identity.generation.0,
        });
    }
    let at_entry = metadata.entries.iter().any(|entry| {
        entry.function == fiber.cursor.function
            && entry.block == fiber.cursor.block
            && fiber.cursor.instruction_offset == 0
            && entry.resume.is_none_or(|resume| {
                program
                    .resume_points
                    .get(resume.index())
                    .is_some_and(|point| {
                        point.function == entry.function && point.block == entry.block
                    })
            })
    });
    if !at_entry || fiber.status != FiberStatus::Running {
        return Err(CompiledApplyError::InvalidEntry);
    }
    Ok(())
}

fn apply_exit(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    exit: CompiledStepExit,
) -> Result<CompiledTransition, CompiledApplyError> {
    match exit {
        CompiledStepExit::Continue { next } => {
            apply_safe_point(program, fiber, next)?;
            Ok(CompiledTransition::Continue)
        }
        CompiledStepExit::HostRequest(request) => {
            validate_resume(program, fiber, request.resume, AwbcSafePointKind::HostCall)?;
            fiber.suspend(FiberSuspension {
                resume: request.resume,
                reason: FiberSuspensionReason::HostCall {
                    call: request.call,
                    args: request.args.clone(),
                    destination: request.destination,
                },
            })?;
            Ok(CompiledTransition::HostRequest(request))
        }
        CompiledStepExit::Suspended(suspension) => {
            let expected = suspension_kind(&suspension.reason);
            validate_resume(program, fiber, suspension.resume, expected)?;
            fiber.suspend(suspension)?;
            Ok(CompiledTransition::Suspended)
        }
        CompiledStepExit::Returned(value) => {
            if fiber.finish_return(program, value.clone())? {
                Ok(CompiledTransition::Returned(value))
            } else {
                Ok(CompiledTransition::Continue)
            }
        }
        CompiledStepExit::BudgetExhausted { resume } => {
            validate_resume(program, fiber, resume, AwbcSafePointKind::BudgetYield)?;
            fiber.suspend(FiberSuspension {
                resume,
                reason: FiberSuspensionReason::BudgetYield,
            })?;
            Ok(CompiledTransition::BudgetExhausted)
        }
        CompiledStepExit::Failed(failure) => {
            fiber.mark_trapped(FiberTrap {
                code: failure.code,
                message: failure.message.clone(),
                source_map: failure.source_map,
            });
            Ok(CompiledTransition::Failed(failure))
        }
        CompiledStepExit::FallbackToVm(fallback) => {
            validate_fallback_point(program, fiber, fallback.at)?;
            Ok(CompiledTransition::FallbackToVm(fallback))
        }
    }
}

fn apply_safe_point(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    point: FiberSafePoint,
) -> Result<(), CompiledApplyError> {
    if point.generation != fiber.generation || point.cursor.instruction_offset != 0 {
        return Err(CompiledApplyError::InvalidSafePoint);
    }
    let frame = fiber.active_frame()?;
    if frame.function != point.cursor.function || frame.layout != point.frame_layout {
        return Err(CompiledApplyError::InvalidSafePoint);
    }
    let block = program
        .blocks
        .get(point.cursor.block.index())
        .ok_or(CompiledApplyError::InvalidSafePoint)?;
    if block.owner != point.cursor.function {
        return Err(CompiledApplyError::InvalidSafePoint);
    }
    if let Some(resume) = point.resume {
        let resume_point = program
            .resume_points
            .get(resume.index())
            .ok_or(CompiledApplyError::InvalidSafePoint)?;
        if resume_point.function != point.cursor.function
            || resume_point.block != point.cursor.block
            || resume_point.frame_layout != point.frame_layout
        {
            return Err(CompiledApplyError::InvalidSafePoint);
        }
    } else if block.safe_point == AwbcSafePointKind::None {
        return Err(CompiledApplyError::InvalidSafePoint);
    }
    fiber.cursor = point.cursor;
    Ok(())
}

fn validate_resume(
    program: &AwbcProgram,
    fiber: &FiberState,
    resume: AwbcResumePointId,
    expected: AwbcSafePointKind,
) -> Result<(), CompiledApplyError> {
    let point = program
        .resume_points
        .get(resume.index())
        .ok_or(CompiledApplyError::InvalidSafePoint)?;
    let frame = fiber.active_frame()?;
    if point.function != frame.function
        || point.frame_layout != frame.layout
        || point.kind != expected
    {
        return Err(CompiledApplyError::InvalidSafePoint);
    }
    Ok(())
}

fn validate_fallback_point(
    program: &AwbcProgram,
    fiber: &FiberState,
    point: FiberSafePoint,
) -> Result<(), CompiledApplyError> {
    if point.generation != fiber.generation || point.cursor != fiber.cursor {
        return Err(CompiledApplyError::InvalidSafePoint);
    }
    let frame = fiber.active_frame()?;
    if point.frame_layout != frame.layout || point.cursor.function != frame.function {
        return Err(CompiledApplyError::InvalidSafePoint);
    }
    if let Some(resume) = point.resume {
        let resume_point = program
            .resume_points
            .get(resume.index())
            .ok_or(CompiledApplyError::InvalidSafePoint)?;
        if resume_point.block != point.cursor.block
            || resume_point.function != point.cursor.function
            || resume_point.frame_layout != point.frame_layout
        {
            return Err(CompiledApplyError::InvalidSafePoint);
        }
    }
    Ok(())
}

const fn suspension_kind(reason: &FiberSuspensionReason) -> AwbcSafePointKind {
    match reason {
        FiberSuspensionReason::Dialogue { .. } => AwbcSafePointKind::Dialogue,
        FiberSuspensionReason::Choice { .. } => AwbcSafePointKind::Choice,
        FiberSuspensionReason::Await { .. } => AwbcSafePointKind::Await,
        FiberSuspensionReason::AwaitMany(_) => AwbcSafePointKind::AwaitMany,
        FiberSuspensionReason::HostCall { .. } => AwbcSafePointKind::HostCall,
        FiberSuspensionReason::BudgetYield => AwbcSafePointKind::BudgetYield,
    }
}
