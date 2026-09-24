//! Compact AWBC VM executor.
//!
//! The VM is Sans I/O. Host operations, line tasks, dialogue, choice, await,
//! await-many and budget yields are surfaced as typed exits over `FiberState`.
//! This module never falls back to the structured VM.

use super::fiber::{
    AwbcProjectCallSite, FiberAwaitManyState, FiberAwaitTarget, FiberCursor, FiberResumeTarget,
    FiberReturnContinuation, FiberReturnPoint, FiberSafePoint, FiberScopeCleanup, FiberState,
    FiberStateError, FiberStatus, FiberSuspension, FiberSuspensionReason, FiberTerminalValue,
    FiberTrap, runtime_value_matches_type, runtime_variant_identity,
};
use super::schema::{
    AwbcBinaryOp, AwbcBlockId, AwbcCodeLocation, AwbcConstant, AwbcConstantId, AwbcContentUnitId,
    AwbcDropPolicy, AwbcEffectPlanId, AwbcFieldProjection, AwbcFunctionId, AwbcInstruction,
    AwbcInstructionId, AwbcIntrinsicId, AwbcLineOperationId, AwbcOpcode, AwbcPattern,
    AwbcPatternId, AwbcPatternRest, AwbcProgram, AwbcProjectCall, AwbcProjectCallAttachedPresence,
    AwbcProjectCallOperandMode, AwbcProjectCallOrdinaryMaterialization, AwbcPureHelperId,
    AwbcRegisterId, AwbcResumePointId, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcSignedIntKind,
    AwbcSourceMapId, AwbcStreamPlanId, AwbcStringId, AwbcTaskPlanId, AwbcTerminator,
    AwbcTraitMethodId, AwbcTraitReceiverMode, AwbcTrapCode, AwbcTypeId, AwbcUnaryOp,
    AwbcUnsignedIntKind,
};
use crate::effect::RuntimeArtifactFingerprint;
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableRetainedRole, RuntimeCallableTransition,
};
use crate::task::{NeedId, RuntimeProgramOwner};
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeAgentValue, RuntimeCallableApplication, RuntimeCallableBodyReference,
    RuntimeCallableInvocation, RuntimeCallableValue, RuntimeFieldValue, RuntimeNominalRecordValue,
    RuntimeRecordValue, RuntimeReductionValue, RuntimeSeq, RuntimeValue, evaluate_binary,
    evaluate_unary, runtime_sequence_from_literal_values, runtime_sequence_repeat_value,
    runtime_value_label,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmStepOptions {
    pub max_instructions: u64,
}

/// Immutable artifact authority required by runtime instructions that package
/// producer-owned opaque values.
#[derive(Clone, Debug)]
pub struct VmExecutionContext {
    artifact: RuntimeArtifactFingerprint,
    program_owner: Option<RuntimeProgramOwner>,
}

impl VmExecutionContext {
    #[must_use]
    pub const fn new(artifact: RuntimeArtifactFingerprint) -> Self {
        Self {
            artifact,
            program_owner: None,
        }
    }

    /// Binds runtime callable construction and activation to the exact shared
    /// AWBC program allocation already leased by the product session.
    #[must_use]
    pub fn for_program(
        artifact: RuntimeArtifactFingerprint,
        program: std::sync::Arc<AwbcProgram>,
    ) -> Self {
        Self {
            artifact,
            program_owner: Some(RuntimeProgramOwner::Awbc(program)),
        }
    }

    #[must_use]
    pub const fn artifact(&self) -> RuntimeArtifactFingerprint {
        self.artifact
    }

    fn program_owner(&self, program: &AwbcProgram) -> Result<RuntimeProgramOwner, VmError> {
        let owner = self
            .program_owner
            .as_ref()
            .ok_or(VmError::MissingExecutionContext)?;
        if !matches!(owner, RuntimeProgramOwner::Awbc(leased) if std::ptr::eq(leased.as_ref(), program))
        {
            return Err(VmError::Runtime(
                "VM execution context leases a different AWBC program".to_owned(),
            ));
        }
        Ok(owner.clone())
    }
}

/// Constructs one dialogue callback from its state row and the already
/// evaluated capture registers. The program table remains the sole owner of
/// callback code and retained-value types.
pub(crate) fn dialogue_effect_callable(
    program: &AwbcProgram,
    owner: RuntimeProgramOwner,
    state_id: crate::runtime_id::RuntimeCallableStateId,
    capture_types: &[AwbcTypeId],
    captures: Vec<RuntimeValue>,
) -> Result<RuntimeCallableValue, VmError> {
    if !matches!(
        &owner,
        RuntimeProgramOwner::Awbc(leased) if std::ptr::eq(leased.as_ref(), program)
    ) {
        return Err(VmError::Runtime(
            "dialogue callback owner does not lease this AWBC program".to_owned(),
        ));
    }
    let state = program
        .callable_states
        .get(state_id.index())
        .ok_or_else(|| VmError::Runtime("dialogue callback state is absent".to_owned()))?;
    if !state.parameters.is_empty()
        || !matches!(state.attached, RuntimeCallableAttachedContract::None)
        || state.retained.len() != capture_types.len()
        || captures.len() != capture_types.len()
        || !matches!(
            program
                .runtime_types
                .get(state.result.index())
                .map(AwbcRuntimeType::shape),
            Some(AwbcRuntimeTypeShape::Unit)
        )
    {
        return Err(VmError::Runtime(
            "dialogue callback state does not have its declared zero-argument Unit ABI".to_owned(),
        ));
    }
    let RuntimeCallableTransition::Invoke {
        function,
        captures: capture_projection,
        arguments,
    } = &state.transition
    else {
        return Err(VmError::Runtime(
            "dialogue callback state does not invoke an executable body".to_owned(),
        ));
    };
    if !arguments.is_empty()
        || capture_projection.len() != capture_types.len()
        || state.retained.iter().zip(capture_types).enumerate().any(
            |(position, (retained, expected))| {
                retained.ty != *expected
                    || retained.role
                        != (RuntimeCallableRetainedRole::Capture {
                            position: u32::try_from(position).unwrap_or(u32::MAX),
                        })
            },
        )
    {
        return Err(VmError::Runtime(
            "dialogue callback retained capture layout disagrees with its manifest".to_owned(),
        ));
    }
    let target = program
        .functions
        .get(function.index())
        .ok_or(VmError::MissingFunction(*function))?;
    if target.kind != super::schema::AwbcFunctionKind::Ordinary {
        return Err(VmError::Runtime(
            "dialogue callback body is not an ordinary function".to_owned(),
        ));
    }
    let signature = program
        .signatures
        .get(target.signature.index())
        .ok_or_else(|| VmError::Runtime("dialogue callback signature is absent".to_owned()))?;
    if signature.result.is_some() || signature.params.as_slice() != capture_types {
        return Err(VmError::Runtime(
            "dialogue callback body signature disagrees with its state".to_owned(),
        ));
    }
    for (position, (value, expected)) in captures.iter().zip(capture_types).enumerate() {
        if !runtime_value_matches_type(program, value, *expected, 0) {
            return Err(VmError::Runtime(format!(
                "dialogue callback capture {position} has the wrong runtime type"
            )));
        }
    }
    RuntimeCallableValue::try_new(owner, state_id, captures)
        .map_err(|error| VmError::Runtime(error.to_string()))
}

impl Default for VmStepOptions {
    fn default() -> Self {
        Self {
            max_instructions: 64,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VmStepOutput {
    pub executed: u64,
    pub observations: Vec<VmObservation>,
    pub exit: VmExit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VmObservation {
    Instruction {
        function: AwbcFunctionId,
        block: AwbcBlockId,
        offset: u32,
        opcode: AwbcOpcode,
    },
    Effect {
        effect: AwbcEffectPlanId,
        args: Vec<RuntimeValue>,
    },
    EnsureContent(AwbcContentUnitId),
    TaskStarted {
        plan: AwbcTaskPlanId,
        handle: RuntimeValue,
        args: Vec<RuntimeValue>,
    },
    Goto(AwbcFunctionId),
    FiberSpawned {
        function: AwbcFunctionId,
        handle: Option<RuntimeValue>,
        args: Vec<RuntimeValue>,
    },
    StreamYield {
        stream: AwbcStreamPlanId,
        value: RuntimeValue,
    },
    StreamClose(AwbcStreamPlanId),
    /// A typed line operation awaiting an atomic Product-runtime transaction.
    ///
    /// The VM deliberately retains `cursor` and does not advance it. The
    /// Product owner must first issue the runtime resource and write `dst`,
    /// then advance this exact cursor as one transaction.
    LineOperation {
        cursor: FiberCursor,
        dst: AwbcRegisterId,
        operation: AwbcLineOperationId,
        args: Vec<(AwbcRegisterId, RuntimeValue)>,
    },
    /// A typed dialogue result awaiting the activation owner's commit.
    ///
    /// As with [`Self::LineOperation`], the instruction remains current until
    /// the Product owner commits the result and advances the exact cursor.
    DialogueResult {
        cursor: FiberCursor,
        source_register: AwbcRegisterId,
        source: RuntimeValue,
    },
    /// Internal affine graph transaction evidence. This never becomes a
    /// host-facing effect; the owning executor reconciles the before/after
    /// register graph with its dialogue handle registry exactly once.
    Drop {
        policy: crate::effect::RuntimeDropPolicy,
    },
    Trap(FiberTrap),
}

#[derive(Clone, Debug, PartialEq)]
pub enum VmExit {
    Running,
    Suspended(FiberSuspensionReason),
    Returned(Option<RuntimeValue>),
    Cancelled,
    Trapped(FiberTrap),
    BudgetYield(FiberSafePoint),
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum VmError {
    #[error("AWBC VM fiber error: {0}")]
    Fiber(#[from] FiberStateError),
    #[error("AWBC function {0:?} does not exist")]
    MissingFunction(AwbcFunctionId),
    #[error("AWBC block {0:?} does not exist")]
    MissingBlock(AwbcBlockId),
    #[error("AWBC instruction {0:?} does not exist")]
    MissingInstruction(AwbcInstructionId),
    #[error("AWBC constant {0:?} does not exist")]
    MissingConstant(AwbcConstantId),
    #[error("AWBC string {0:?} does not exist")]
    MissingString(AwbcStringId),
    #[error("AWBC instruction requires an artifact-bound VM execution context")]
    MissingExecutionContext,
    #[error("AWBC pattern {0:?} does not exist")]
    MissingPattern(AwbcPatternId),
    #[error("AWBC runtime type {0:?} does not exist")]
    MissingType(AwbcTypeId),
    #[error("AWBC intrinsic {0:?} was not resolved by the VM host")]
    MissingIntrinsic(AwbcIntrinsicId),
    #[error("AWBC trait method {0:?} does not exist")]
    MissingTraitMethod(AwbcTraitMethodId),
    #[error("function application expected {expected} arguments, received {actual}")]
    FunctionArgumentCount { expected: usize, actual: usize },
    #[error("runtime error: {0}")]
    Runtime(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstructionControl {
    Continue,
    Transferred,
    /// Commits the instruction-local fiber mutation, advances its exact
    /// cursor once, and returns the observation batch to the owning runtime
    /// before any later instruction can execute.
    YieldAdvanced,
    /// Leaves the cursor parked for an external two-phase owner to commit.
    Yield,
}

pub trait VmHost {
    fn call_intrinsic(
        &mut self,
        program: &AwbcProgram,
        intrinsic: AwbcIntrinsicId,
        args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError>;

    fn call_pure_helper(
        &mut self,
        program: &AwbcProgram,
        helper: AwbcPureHelperId,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, VmError>;
}

#[derive(Clone, Debug, Default)]
pub struct RejectingVmHost;

impl VmHost for RejectingVmHost {
    fn call_intrinsic(
        &mut self,
        _program: &AwbcProgram,
        intrinsic: AwbcIntrinsicId,
        _args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError> {
        Err(VmError::MissingIntrinsic(intrinsic))
    }

    fn call_pure_helper(
        &mut self,
        _program: &AwbcProgram,
        helper: AwbcPureHelperId,
        _args: &[RuntimeValue],
    ) -> Result<RuntimeValue, VmError> {
        Err(VmError::Runtime(format!(
            "pure helper {} is not bound",
            helper.0
        )))
    }
}

pub fn step(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    options: VmStepOptions,
) -> Result<VmStepOutput, VmError> {
    let mut host = RejectingVmHost;
    step_with_host(program, fiber, options, &mut host)
}

/// Cancels a live fiber and emits its registered cleanups in unwind order.
///
/// Terminal fibers are stable: a later cancellation cannot replace their
/// result or replay already-detached cleanups.
pub fn cancel_fiber(fiber: &mut FiberState) -> VmStepOutput {
    let mut observations = Vec::new();
    if matches!(fiber.status, FiberStatus::Running | FiberStatus::Suspended) {
        emit_ordered_cleanup_observations(fiber.take_unwind_cleanups(), &mut observations);
        fiber.mark_cancelled();
    }
    VmStepOutput {
        executed: 0,
        observations,
        exit: terminal_exit(fiber),
    }
}

#[allow(clippy::too_many_lines)]
pub fn step_with_host(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    options: VmStepOptions,
    host: &mut impl VmHost,
) -> Result<VmStepOutput, VmError> {
    step_with_host_context_optional(program, fiber, options, None, host)
}

/// Executes a bounded AWBC slice with the exact artifact context used for
/// producer-owned opaque value construction.
pub fn step_with_host_context(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    options: VmStepOptions,
    context: &VmExecutionContext,
    host: &mut impl VmHost,
) -> Result<VmStepOutput, VmError> {
    step_with_host_context_optional(program, fiber, options, Some(context), host)
}

fn step_with_host_context_optional(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    options: VmStepOptions,
    context: Option<&VmExecutionContext>,
    host: &mut impl VmHost,
) -> Result<VmStepOutput, VmError> {
    let mut observations = Vec::new();
    let mut executed = 0_u64;
    while fiber.status == FiberStatus::Running && executed < options.max_instructions {
        if !fiber.consume_budget(1) {
            let safe_point = fiber.safe_point(None)?;
            fiber.suspend(FiberSuspension {
                resume: FiberResumeTarget::Exact(safe_point.cursor),
                reason: FiberSuspensionReason::BudgetYield,
            })?;
            return Ok(VmStepOutput {
                executed,
                observations,
                exit: VmExit::BudgetYield(safe_point),
            });
        }
        let cursor = fiber.cursor;
        let block = program
            .blocks
            .get(cursor.block.index())
            .ok_or(VmError::MissingBlock(cursor.block))?;
        let instruction_index = block
            .instructions
            .start
            .saturating_add(cursor.instruction_offset);
        if cursor.instruction_offset < block.instructions.len {
            let instruction_id = AwbcInstructionId(instruction_index);
            let instruction = program
                .instructions
                .get(instruction_id.index())
                .ok_or(VmError::MissingInstruction(instruction_id))?;
            observations.push(VmObservation::Instruction {
                function: cursor.function,
                block: cursor.block,
                offset: cursor.instruction_offset,
                opcode: instruction.opcode(),
            });
            let source_map =
                source_map_for_location(program, AwbcCodeLocation::Instruction(instruction_id))
                    .or(block.source_map);
            let control = match execute_instruction(
                program,
                fiber,
                host,
                context,
                instruction,
                source_map,
                &mut observations,
            ) {
                Ok(control) => control,
                Err(error) => {
                    if let Some(code) = error.runtime_trap_code() {
                        let trap = mark_runtime_error_trap(
                            fiber,
                            code,
                            error.to_string(),
                            source_map,
                            &mut observations,
                        );
                        executed = executed.saturating_add(1);
                        return Ok(VmStepOutput {
                            executed,
                            observations,
                            exit: VmExit::Trapped(trap),
                        });
                    }
                    return Err(error);
                }
            };
            match control {
                InstructionControl::Continue => {
                    if fiber.cursor != cursor {
                        return Err(VmError::Runtime(
                            "instruction changed control flow without reporting a transfer"
                                .to_owned(),
                        ));
                    }
                    fiber.cursor.instruction_offset =
                        cursor.instruction_offset.checked_add(1).ok_or_else(|| {
                            VmError::Runtime("AWBC instruction cursor overflowed".to_owned())
                        })?;
                }
                InstructionControl::Transferred => {}
                InstructionControl::YieldAdvanced => {
                    if fiber.cursor != cursor {
                        return Err(VmError::Runtime(
                            "yielding instruction changed its execution cursor".to_owned(),
                        ));
                    }
                    fiber.cursor.instruction_offset =
                        cursor.instruction_offset.checked_add(1).ok_or_else(|| {
                            VmError::Runtime("AWBC instruction cursor overflowed".to_owned())
                        })?;
                    executed = executed.saturating_add(1);
                    return Ok(VmStepOutput {
                        executed,
                        observations,
                        exit: VmExit::Running,
                    });
                }
                InstructionControl::Yield => {
                    if fiber.cursor != cursor {
                        return Err(VmError::Runtime(
                            "yielding instruction changed its execution cursor".to_owned(),
                        ));
                    }
                    executed = executed.saturating_add(1);
                    return Ok(VmStepOutput {
                        executed,
                        observations,
                        exit: VmExit::Running,
                    });
                }
            }
            executed = executed.saturating_add(1);
            continue;
        }
        let source_map = block
            .source_map
            .or_else(|| source_map_for_location(program, AwbcCodeLocation::Block(cursor.block)));
        let exit = match execute_terminator(
            program,
            fiber,
            host,
            context,
            &block.terminator,
            source_map,
            &mut observations,
        ) {
            Ok(exit) => exit,
            Err(error) => {
                if let Some(code) = error.runtime_trap_code() {
                    let trap = mark_runtime_error_trap(
                        fiber,
                        code,
                        error.to_string(),
                        source_map,
                        &mut observations,
                    );
                    VmExit::Trapped(trap)
                } else {
                    return Err(error);
                }
            }
        };
        executed = executed.saturating_add(1);
        if !matches!(exit, VmExit::Running) {
            return Ok(VmStepOutput {
                executed,
                observations,
                exit,
            });
        }
    }
    Ok(VmStepOutput {
        executed,
        observations,
        exit: terminal_exit(fiber),
    })
}

#[allow(clippy::too_many_lines)]
fn execute_instruction(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    host: &mut impl VmHost,
    context: Option<&VmExecutionContext>,
    instruction: &AwbcInstruction,
    source_map: Option<AwbcSourceMapId>,
    observations: &mut Vec<VmObservation>,
) -> Result<InstructionControl, VmError> {
    match instruction {
        AwbcInstruction::Nop => {}
        AwbcInstruction::LoadConst { dst, constant } => {
            let value = constant_value(program, *constant)?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::Move { dst, src } => {
            if dst == src {
                register(fiber, *src)?;
            } else {
                let frame = fiber.active_frame_mut()?;
                let mut registers = frame.registers.clone();
                let value = registers
                    .get_mut(src.index())
                    .and_then(Option::take)
                    .ok_or(FiberStateError::RegisterOutOfBounds {
                        register: src.0,
                        layout: frame.layout.0,
                    })?;
                let destination =
                    registers
                        .get_mut(dst.index())
                        .ok_or(FiberStateError::RegisterOutOfBounds {
                            register: dst.0,
                            layout: frame.layout.0,
                        })?;
                *destination = Some(value);
                frame.registers = registers;
            }
        }
        AwbcInstruction::CopyValue { dst, src } => {
            let value = register(fiber, *src)?;
            if !value.ownership().permits_copy() {
                return Err(VmError::Runtime(
                    "CopyValue rejected an affine runtime value graph".to_owned(),
                ));
            }
            let value = value.clone();
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::Clear {
            register: register_id,
        } => {
            if !register(fiber, *register_id)?.ownership().permits_copy() {
                return Err(VmError::Runtime(
                    "Clear rejected an affine runtime value graph".to_owned(),
                ));
            }
            fiber.active_frame_mut()?.clear_register(*register_id)?;
        }
        AwbcInstruction::Drop {
            register: register_id,
            policy,
        } => {
            let policy = materialize_drop_policy(fiber, *policy)?;
            fiber.active_frame_mut()?.clear_register(*register_id)?;
            observations.push(VmObservation::Drop { policy });
            return Ok(InstructionControl::YieldAdvanced);
        }
        AwbcInstruction::EnterScope { scope } => {
            let frame = fiber.active_frame()?;
            let definition = program
                .frame_layouts
                .get(frame.layout.index())
                .and_then(|layout| layout.scopes.get(scope.index()))
                .ok_or_else(|| VmError::Runtime("scope has no frame definition".to_owned()))?;
            if definition.parent != frame.scopes.last().map(|scope| scope.id)
                || frame.scopes.iter().any(|active| active.id == *scope)
            {
                return Err(VmError::Runtime(
                    "scope does not match the active lexical parent".to_owned(),
                ));
            }
            let depth = u32::try_from(fiber.active_frame()?.scopes.len())
                .map_err(|_| VmError::Runtime("scope depth exceeds u32".to_owned()))?;
            fiber
                .active_frame_mut()?
                .scopes
                .push(super::fiber::FiberScope {
                    id: *scope,
                    depth,
                    cleanups: Vec::new(),
                });
        }
        AwbcInstruction::ExitScope { scope } => {
            if fiber.active_frame()?.scopes.last().map(|active| active.id) != Some(*scope) {
                return Err(VmError::Runtime(
                    "scope exit does not match the active scope".to_owned(),
                ));
            }
            let layout_id = fiber.active_frame()?.layout;
            let layout = program
                .frame_layouts
                .get(layout_id.index())
                .ok_or(FiberStateError::UnknownFrameLayout(layout_id.0))?;
            let frame = fiber.active_frame_mut()?;
            if let Some(scope) = frame.scopes.pop() {
                emit_cleanup_observations(scope.cleanups, observations);
            }
            let active_scope_depth = u32::try_from(frame.scopes.len())
                .map_err(|_| VmError::Runtime("scope depth exceeds u32".to_owned()))?;
            for (register, slot) in frame.registers.iter_mut().zip(&layout.slots) {
                if slot.scope_depth > active_scope_depth
                    && !matches!(
                        slot.role,
                        super::schema::AwbcFrameSlotRole::Parameter
                            | super::schema::AwbcFrameSlotRole::RuntimeState
                    )
                {
                    *register = None;
                }
            }
        }
        AwbcInstruction::BindPattern { pattern, value, .. } => {
            let value = register(fiber, *value)?.clone();
            bind_pattern(program, fiber, *pattern, &value)?;
        }
        AwbcInstruction::TestPattern {
            dst,
            pattern,
            value,
        } => {
            let value = register(fiber, *value)?.clone();
            let matched = test_pattern(program, *pattern, &value)?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Bool(matched))?;
        }
        AwbcInstruction::MakeTuple { dst, items } => {
            let items = items
                .iter()
                .map(|item| register(fiber, *item).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Tuple(items))?;
        }
        AwbcInstruction::MakeSequence { dst, items } => {
            let items = items
                .iter()
                .map(|item| register(fiber, *item).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, runtime_sequence_from_literal_values(items))?;
        }
        AwbcInstruction::RepeatSequence { dst, value, len } => {
            let value = register(fiber, *value)?.clone();
            let len = usize::try_from(register(fiber, *len)?.try_u64().unwrap_or_default())
                .unwrap_or(usize::MAX);
            fiber
                .active_frame_mut()?
                .set_register(*dst, runtime_sequence_repeat_value(&value, len))?;
        }
        AwbcInstruction::SequenceLen { dst, sequence } => {
            let len = {
                let RuntimeValue::Seq(sequence) = register(fiber, *sequence)? else {
                    return Err(VmError::Runtime(
                        "sequence length expected a sequence".to_owned(),
                    ));
                };
                sequence.len() as u64
            };
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::usize(len))?;
        }
        AwbcInstruction::SequenceGet {
            dst,
            sequence,
            index,
        } => {
            let index = usize::try_from(register(fiber, *index)?.try_u64().unwrap_or(u64::MAX))
                .unwrap_or(usize::MAX);
            let value = {
                let RuntimeValue::Seq(sequence) = register(fiber, *sequence)? else {
                    return Err(VmError::Runtime(
                        "sequence get expected a sequence".to_owned(),
                    ));
                };
                if index >= sequence.len() {
                    trap(
                        fiber,
                        AwbcTrapCode::InvalidIndex,
                        Some("sequence index out of bounds"),
                        source_map,
                        observations,
                    );
                    return Ok(InstructionControl::Continue);
                }
                sequence.value_at(index)
            };
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::SequenceSlice {
            dst,
            sequence,
            start,
        } => {
            let start = usize::try_from(register(fiber, *start)?.try_u64().unwrap_or_default())
                .unwrap_or(usize::MAX);
            let tail = {
                let RuntimeValue::Seq(sequence) = register(fiber, *sequence)? else {
                    return Err(VmError::Runtime(
                        "sequence slice expected a sequence".to_owned(),
                    ));
                };
                sequence.tail_from(start)
            };
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Seq(tail))?;
        }
        AwbcInstruction::SequencePush { sequence, value } => {
            let value = register(fiber, *value)?.clone();
            let frame = fiber.active_frame_mut()?;
            match frame
                .registers
                .get_mut(sequence.index())
                .and_then(Option::as_mut)
            {
                Some(RuntimeValue::Seq(RuntimeSeq::Values(values))) => values.push(value),
                Some(value_ref) => {
                    let existing = value_ref.clone();
                    *value_ref = runtime_sequence_from_literal_values(vec![existing, value]);
                }
                None => {
                    return Err(FiberStateError::RegisterOutOfBounds {
                        register: sequence.0,
                        layout: frame.layout.0,
                    }
                    .into());
                }
            }
        }
        AwbcInstruction::MakeRecord { dst, ty, fields } => {
            let fields = fields
                .iter()
                .map(|register_id| register(fiber, *register_id).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            let value = program.make_record_value(*ty, fields)?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::MakeVariant {
            dst,
            ty,
            case,
            case_name,
            payload,
        } => {
            let payload = payload
                .map(|payload| register(fiber, payload).cloned())
                .transpose()?
                .map(Box::new);
            fiber.active_frame_mut()?.set_register(
                *dst,
                RuntimeValue::Variant {
                    owner: variant_identity_for_type(program, *ty)?,
                    ordinal: *case,
                    name: string(program, *case_name)?.to_owned(),
                    payload,
                },
            )?;
        }
        AwbcInstruction::MakeAgent {
            dst,
            constructor,
            operands,
        } => {
            let operands = operands
                .iter()
                .map(|operand| register(fiber, *operand).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            let value = RuntimeAgentValue::try_construct(*constructor, operands)
                .map(RuntimeValue::Agent)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::MakeReductionUnchanged { dst, ty, state } => {
            let owner = program
                .opaque_owner(*ty)
                .map_err(|error| VmError::Runtime(error.to_string()))?
                .ok_or_else(|| {
                    VmError::Runtime("Reduction requires an opaque runtime type".to_owned())
                })?;
            let state = register(fiber, *state)?.clone();
            let value = RuntimeReductionValue::try_unchanged(owner, state)
                .map(RuntimeValue::Reduction)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::ProjectTuple {
            dst,
            target,
            ordinal,
        } => {
            let RuntimeValue::Tuple(items) = register(fiber, *target)? else {
                return Err(VmError::Runtime(
                    "tuple projection expected tuple".to_owned(),
                ));
            };
            let value = items
                .get(*ordinal as usize)
                .cloned()
                .ok_or_else(|| VmError::Runtime("tuple projection out of bounds".to_owned()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::ProjectRecord {
            dst,
            target,
            ordinal,
        } => {
            let field =
                crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(*ordinal as usize)
                    .map_err(|error| VmError::Runtime(error.to_string()))?;
            let value = register(fiber, *target)?
                .record_field(field)
                .cloned()
                .ok_or_else(|| VmError::Runtime("record projection out of bounds".to_owned()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::ProjectField { dst, target, field } => {
            let value = match field {
                AwbcFieldProjection::Named(field) => {
                    let field = string(program, *field)?;
                    match register(fiber, *target)? {
                        RuntimeValue::Record(items) => items
                            .iter()
                            .find(|item| item.name() == field)
                            .map(|field| field.value().clone()),
                        RuntimeValue::Agent(value) => value.project_field_label(field),
                        RuntimeValue::Progress(progress) => match field {
                            "ratio" => Some(RuntimeValue::F32(progress.ratio())),
                            "label" => Some(progress.label().map_or_else(
                                RuntimeValue::option_none,
                                |label| {
                                    RuntimeValue::option_some(RuntimeValue::String(
                                        label.to_owned(),
                                    ))
                                },
                            )),
                            _ => None,
                        },
                        _ => None,
                    }
                    .ok_or_else(|| VmError::Runtime(format!("missing field `{field}`")))?
                }
                AwbcFieldProjection::OpaqueRecord {
                    owner,
                    field,
                    field_type,
                } => {
                    let owner = program
                        .opaque_owner(*owner)
                        .map_err(|error| VmError::Runtime(error.to_string()))?
                        .ok_or_else(|| {
                            VmError::Runtime(
                                "opaque-record projection requires an opaque owner type".to_owned(),
                            )
                        })?;
                    let RuntimeValue::Opaque(value) = register(fiber, *target)? else {
                        return Err(VmError::Runtime(
                            "opaque-record projection expected an opaque value".to_owned(),
                        ));
                    };
                    if !owner.accepts_opaque_value(value) {
                        return Err(VmError::Runtime(
                            "opaque-record projection rejected the target owner".to_owned(),
                        ));
                    }
                    let RuntimeValue::Tuple(fields) = value.payload() else {
                        return Err(VmError::Runtime(
                            "opaque-record projection expected a tuple payload".to_owned(),
                        ));
                    };
                    let value = fields.get(*field as usize).cloned().ok_or_else(|| {
                        VmError::Runtime("opaque-record projection out of bounds".to_owned())
                    })?;
                    if !runtime_value_matches_type(program, &value, *field_type, 0) {
                        return Err(VmError::Runtime(
                            "opaque-record projection rejected the field value type".to_owned(),
                        ));
                    }
                    value
                }
            };
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::Unary { dst, op, src } => {
            let value = register(fiber, *src)?.clone();
            let value = evaluate_unary(unary_op(*op), value)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::Binary { dst, op, lhs, rhs } => {
            let lhs = register(fiber, *lhs)?.clone();
            let rhs = register(fiber, *rhs)?.clone();
            let value = evaluate_binary(lhs, binary_op(*op), rhs)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::CallPureHelper { dst, helper, args } => {
            let args = args
                .iter()
                .map(|arg| register(fiber, *arg).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            let value = host.call_pure_helper(program, *helper, &args)?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::AssignRecordField {
            target,
            field,
            value,
        } => {
            let value = register(fiber, *value)?.clone();
            let frame = fiber.active_frame_mut()?;
            let Some(target_value) = frame
                .registers
                .get_mut(target.index())
                .and_then(Option::as_mut)
            else {
                return Err(FiberStateError::RegisterOutOfBounds {
                    register: target.0,
                    layout: frame.layout.0,
                }
                .into());
            };
            set_record_field_value(target_value, *field, value)?;
        }
        AwbcInstruction::CallTraitMethod {
            dst,
            method,
            receiver,
            args,
            receiver_out,
        } => {
            let outcome =
                execute_trait_method_call(program, fiber, host, context, *method, *receiver, args)?;
            let TraitMethodCallOutcome::Completed(outcome) = outcome else {
                return Ok(InstructionControl::Transferred);
            };
            fiber
                .active_frame_mut()?
                .set_register(*dst, outcome.value)?;
            if let (Some(register), Some(updated_receiver)) =
                (*receiver_out, outcome.updated_receiver)
            {
                fiber
                    .active_frame_mut()?
                    .set_register(register, updated_receiver)?;
            }
        }
        AwbcInstruction::CallIntrinsic {
            dst,
            intrinsic,
            args,
        } => {
            let args = args
                .iter()
                .map(|arg| register(fiber, *arg).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(value) = host.call_intrinsic(program, *intrinsic, &args)?
                && let Some(dst) = dst
            {
                fiber.active_frame_mut()?.set_register(*dst, value)?;
            }
        }
        AwbcInstruction::EnsureContent { content } => {
            observations.push(VmObservation::EnsureContent(*content));
        }
        AwbcInstruction::MakeDialogueContent {
            destination,
            template,
            values,
            effects,
        } => {
            let context = context.ok_or(VmError::MissingExecutionContext)?;
            let template = program
                .content_templates
                .iter()
                .find(|candidate| candidate.id == *template)
                .ok_or_else(|| {
                    VmError::Runtime(
                        "MakeDialogueContent references a missing template manifest".to_owned(),
                    )
                })?;
            if template.slots.len() != values.len() {
                return Err(VmError::FunctionArgumentCount {
                    expected: template.slots.len(),
                    actual: values.len(),
                });
            }
            let evaluated = values
                .iter()
                .enumerate()
                .map(|(index, binding)| {
                    let slot = template.slots.get(index).ok_or_else(|| {
                        VmError::Runtime("dialogue content template slot is absent".to_owned())
                    })?;
                    let expected_slot = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index)
                        .ok_or_else(|| {
                            VmError::Runtime(
                                "dialogue content value slot exceeds the identity domain"
                                    .to_owned(),
                            )
                        })?;
                    if binding.slot != expected_slot || binding.slot != slot.slot {
                        return Err(VmError::Runtime(
                            "dialogue content value binding does not match its canonical template slot"
                                .to_owned(),
                        ));
                    }
                    let value = register(fiber, binding.value)?.clone();
                    let role = match slot.role {
                        super::schema::AwbcDialogueValueRole::Interpolation => {
                            crate::plan::RuntimeDialogueValueRole::Interpolation
                        }
                        super::schema::AwbcDialogueValueRole::Content => {
                            crate::plan::RuntimeDialogueValueRole::Content
                        }
                    };
                    Ok(crate::plan::RuntimeDialogueValueBinding {
                        slot: slot.slot,
                        role,
                        value,
                    })
                })
                .collect::<Result<Vec<_>, VmError>>()?;
            if template.effects.len() != effects.len() {
                return Err(VmError::FunctionArgumentCount {
                    expected: template.effects.len(),
                    actual: effects.len(),
                });
            }
            let effect_bindings = effects
                .iter()
                .enumerate()
                .map(|(index, binding)| {
                    let slot = template.effects.get(index).ok_or_else(|| {
                        VmError::Runtime("dialogue content effect slot is absent".to_owned())
                    })?;
                    let expected_site =
                        crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                            .ok_or_else(|| {
                                VmError::Runtime(
                                    "dialogue content effect site exceeds the identity domain"
                                        .to_owned(),
                                )
                            })?;
                    if binding.site != expected_site || binding.site != slot.site {
                        return Err(VmError::Runtime(
                            "dialogue content effect binding does not match its canonical template effect slot"
                                .to_owned(),
                        ));
                    }
                    if binding.captures.len() != slot.capture_types.len() {
                        return Err(VmError::FunctionArgumentCount {
                            expected: slot.capture_types.len(),
                            actual: binding.captures.len(),
                        });
                    }
                    let captures = binding
                        .captures
                        .iter()
                        .map(|register_id| register(fiber, *register_id).cloned())
                        .collect::<Result<Vec<_>, VmError>>()?;
                    let owner = context.program_owner(program)?;
                    let callback = dialogue_effect_callable(
                        program,
                        owner,
                        binding.state,
                        &slot.capture_types,
                        captures,
                    )?;
                    Ok(crate::value::RuntimeDialogueContentEffectBinding::new(
                        binding.site,
                        callback,
                    ))
                })
                .collect::<Result<Vec<_>, VmError>>()?;
            let slots = template
                .slots
                .iter()
                .map(|slot| {
                    let semantic_type = program
                        .runtime_types
                        .get(slot.semantic_type.index())
                        .ok_or(VmError::MissingType(slot.semantic_type))?
                        .semantic_identity();
                    let role = match slot.role {
                        super::schema::AwbcDialogueValueRole::Interpolation => {
                            crate::plan::RuntimeDialogueValueRole::Interpolation
                        }
                        super::schema::AwbcDialogueValueRole::Content => {
                            crate::plan::RuntimeDialogueValueRole::Content
                        }
                    };
                    Ok(crate::plan::RuntimeDialogueContentSlot::new(
                        slot.slot,
                        role,
                        semantic_type,
                    ))
                })
                .collect::<Result<Vec<_>, VmError>>()?;
            let value = crate::value::RuntimeDialogueContentValue::try_from_evaluated_bindings_parts_with_effect_bindings(
                    context.artifact(),
                    template.id,
                    template.digest,
                    &slots,
                    &evaluated,
                    &effect_bindings,
                )
                .map_err(|error| VmError::Runtime(error.to_string()))?
                .into_runtime_value();
            fiber
                .active_frame_mut()?
                .set_register(*destination, value)?;
        }
        AwbcInstruction::EmitEffect { effect, args } => {
            let args = register_values(fiber, args)?;
            observations.push(VmObservation::Effect {
                effect: *effect,
                args,
            });
        }
        AwbcInstruction::RegisterCleanup { key, effect, args } => {
            let key = string(program, *key)?.to_owned();
            let args = register_values(fiber, args)?;
            let cleanup = FiberScopeCleanup {
                key,
                effect: *effect,
                args,
            };
            let frame = fiber.active_frame_mut()?;
            if let Some(scope) = frame.scopes.last_mut() {
                scope.cleanups.push(cleanup);
            } else {
                frame.root_cleanups.push(cleanup);
            }
        }
        AwbcInstruction::CancelCleanup { key } => {
            let key = string(program, *key)?;
            let frame = fiber.active_frame_mut()?;
            frame.root_cleanups.retain(|cleanup| cleanup.key != key);
            for scope in &mut frame.scopes {
                scope.cleanups.retain(|cleanup| cleanup.key != key);
            }
        }
        AwbcInstruction::MakeCallable {
            dst,
            state,
            captures,
        } => {
            let owner = context
                .ok_or(VmError::MissingExecutionContext)?
                .program_owner(program)?;
            let captures = register_values(fiber, captures)?;
            let callable = RuntimeCallableValue::try_new(owner, *state, captures)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Callable(callable))?;
        }
        AwbcInstruction::ApplyGroup { dst, callee, args } => {
            let callee = register(fiber, *callee)?.clone();
            let args = register_values(fiber, args)?;
            let RuntimeValue::Callable(callable) = callee else {
                return Err(VmError::Runtime(format!(
                    "callable application expected callable, found {}",
                    runtime_value_label(&callee)
                )));
            };
            return apply_runtime_callable(program, fiber, context, &callable, &args, *dst);
        }
        AwbcInstruction::StartTask { dst, plan, args } => {
            let args = register_values(fiber, args)?;
            let handle = RuntimeValue::String(
                program
                    .task_plans
                    .get(plan.index())
                    .and_then(|plan| program.strings.get(plan.public_id.index()))
                    .cloned()
                    .unwrap_or_else(|| format!("awbc.task.{}", plan.0)),
            );
            fiber
                .active_frame_mut()?
                .set_register(*dst, handle.clone())?;
            observations.push(VmObservation::TaskStarted {
                plan: *plan,
                handle,
                args,
            });
        }
        AwbcInstruction::SpawnFiber {
            dst,
            function,
            args,
        } => {
            let args = register_values(fiber, args)?;
            let handle = dst.map(|_| RuntimeValue::String(format!("awbc.fiber.{}", function.0)));
            if let (Some(dst), Some(handle)) = (dst, handle.as_ref()) {
                fiber
                    .active_frame_mut()?
                    .set_register(*dst, handle.clone())?;
            }
            observations.push(VmObservation::FiberSpawned {
                function: *function,
                handle,
                args,
            });
        }
        AwbcInstruction::StreamYield { stream, value } => {
            let value = register(fiber, *value)?.clone();
            observations.push(VmObservation::StreamYield {
                stream: *stream,
                value: value.clone(),
            });
            if let Some(state) = fiber.streams.iter_mut().find(|state| state.plan == *stream) {
                state.queue.push(value);
                state.emitted_count = state.emitted_count.saturating_add(1);
            }
        }
        AwbcInstruction::StreamClose { stream } => {
            if let Some(state) = fiber.streams.iter_mut().find(|state| state.plan == *stream)
                && !state.closed
            {
                state.closed = true;
                observations.push(VmObservation::StreamClose(*stream));
            }
        }
        AwbcInstruction::ExecuteLineOperation {
            dst,
            operation,
            args,
        } => {
            observations.push(VmObservation::LineOperation {
                cursor: fiber.cursor,
                dst: *dst,
                operation: *operation,
                args: args
                    .iter()
                    .copied()
                    .map(|slot| Ok((slot, register(fiber, slot)?.clone())))
                    .collect::<Result<_, VmError>>()?,
            });
            return Ok(InstructionControl::Yield);
        }
        AwbcInstruction::CommitDialogueResult { source } => {
            observations.push(VmObservation::DialogueResult {
                cursor: fiber.cursor,
                source_register: *source,
                source: register(fiber, *source)?.clone(),
            });
            return Ok(InstructionControl::Yield);
        }
    }
    Ok(InstructionControl::Continue)
}

fn drain_active_frame_cleanups(
    fiber: &mut FiberState,
    observations: &mut Vec<VmObservation>,
) -> Result<(), VmError> {
    emit_ordered_cleanup_observations(fiber.take_active_frame_cleanups()?, observations);
    Ok(())
}

fn emit_cleanup_observations(
    mut cleanups: Vec<FiberScopeCleanup>,
    observations: &mut Vec<VmObservation>,
) {
    while let Some(cleanup) = cleanups.pop() {
        observations.push(VmObservation::Effect {
            effect: cleanup.effect,
            args: cleanup.args,
        });
    }
}

fn emit_ordered_cleanup_observations(
    cleanups: Vec<FiberScopeCleanup>,
    observations: &mut Vec<VmObservation>,
) {
    for cleanup in cleanups {
        observations.push(VmObservation::Effect {
            effect: cleanup.effect,
            args: cleanup.args,
        });
    }
}

fn emit_unwind_cleanup_observations(fiber: &mut FiberState, observations: &mut Vec<VmObservation>) {
    emit_ordered_cleanup_observations(fiber.take_unwind_cleanups(), observations);
}

fn apply_runtime_callable(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    context: Option<&VmExecutionContext>,
    callable: &RuntimeCallableValue,
    args: &[RuntimeValue],
    destination: AwbcRegisterId,
) -> Result<InstructionControl, VmError> {
    let owner = context
        .ok_or(VmError::MissingExecutionContext)?
        .program_owner(program)?;
    callable
        .validate_for_owner(&owner)
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    let arity = callable
        .remaining_arity()
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    if args.len() < arity {
        let partial = callable
            .try_bind_prefix(args)
            .map_err(|error| VmError::Runtime(error.to_string()))?;
        fiber
            .active_frame_mut()?
            .set_register(destination, RuntimeValue::Callable(partial))?;
        return Ok(InstructionControl::Continue);
    }
    if args.len() > arity {
        return Err(VmError::FunctionArgumentCount {
            expected: arity,
            actual: args.len(),
        });
    }
    let (logical_arguments, attached) = callable
        .materialize_abi_arguments(args)
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    let application = callable
        .prepare_group(&logical_arguments, attached)
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    let caller = fiber.cursor;
    let return_cursor = FiberCursor {
        function: caller.function,
        block: caller.block,
        instruction_offset: caller.instruction_offset.saturating_add(1),
    };
    match application {
        RuntimeCallableApplication::Complete(value) => {
            fiber.active_frame_mut()?.set_register(destination, value)?;
            Ok(InstructionControl::Continue)
        }
        RuntimeCallableApplication::Invoke(invocation) => {
            let function = invocation_function(&invocation)?;
            let values = invocation_values(invocation)?;
            let return_to = FiberReturnPoint::ordinary(return_cursor, Some(destination));
            fiber.push_call_frame_at(program, function, return_to, &values)?;
            Ok(InstructionControl::Transferred)
        }
        RuntimeCallableApplication::AttachedDefault(invocation) => {
            let function = invocation_function(&invocation)?;
            let values = invocation_values(invocation)?;
            let return_to = FiberReturnPoint {
                cursor: return_cursor,
                destination: None,
                continuation: FiberReturnContinuation::ApplyGroupDefault {
                    callable: RuntimeValue::Callable(callable.clone()),
                    arguments: logical_arguments,
                    destination,
                },
            };
            fiber.push_call_frame_with_continuation(program, function, return_to, &values)?;
            Ok(InstructionControl::Transferred)
        }
    }
}

fn invocation_function(invocation: &RuntimeCallableInvocation) -> Result<AwbcFunctionId, VmError> {
    match invocation.body {
        RuntimeCallableBodyReference::Awbc(function) => Ok(function),
        RuntimeCallableBodyReference::Plan(_) => Err(VmError::Runtime(
            "an AWBC callable selected a structured function body".to_owned(),
        )),
    }
}

fn invocation_values(invocation: RuntimeCallableInvocation) -> Result<Vec<RuntimeValue>, VmError> {
    let mut values = invocation.captures;
    values.extend(invocation.arguments);
    Ok(values)
}

#[derive(Debug)]
struct TraitMethodVmOutcome {
    value: RuntimeValue,
    updated_receiver: Option<RuntimeValue>,
}

#[derive(Debug)]
enum TraitMethodCallOutcome {
    Completed(TraitMethodVmOutcome),
    BudgetYield,
}

fn execute_trait_method_call(
    program: &AwbcProgram,
    caller: &mut FiberState,
    host: &mut impl VmHost,
    context: Option<&VmExecutionContext>,
    method: AwbcTraitMethodId,
    receiver: AwbcRegisterId,
    args: &[AwbcRegisterId],
) -> Result<TraitMethodCallOutcome, VmError> {
    const TRAIT_METHOD_BUDGET: u64 = 4_096;

    let method_record = program
        .trait_methods
        .get(method.index())
        .ok_or(VmError::MissingTraitMethod(method))?;
    let mut values = Vec::with_capacity(args.len() + 1);
    values.push(register(caller, receiver)?.clone());
    values.extend(register_values(caller, args)?);

    let mut method_fiber = FiberState::for_function(
        program,
        caller.entry,
        method_record.function,
        caller.generation,
        TRAIT_METHOD_BUDGET,
    )?;
    method_fiber
        .active_frame_mut()?
        .bind_positional_arguments(program, &values)?;

    let mut executed = 0_u64;
    loop {
        let output = step_with_host_context_optional(
            program,
            &mut method_fiber,
            VmStepOptions {
                max_instructions: TRAIT_METHOD_BUDGET,
            },
            context,
            host,
        )?;
        executed = executed.saturating_add(output.executed);
        match output.exit {
            VmExit::Running if executed < TRAIT_METHOD_BUDGET => {}
            VmExit::Returned(Some(value)) => {
                if executed > 0 && !caller.consume_budget(executed) {
                    let safe_point = caller.safe_point(None)?;
                    caller.suspend(FiberSuspension {
                        resume: FiberResumeTarget::Exact(safe_point.cursor),
                        reason: FiberSuspensionReason::BudgetYield,
                    })?;
                    return Ok(TraitMethodCallOutcome::BudgetYield);
                }
                let updated_receiver = if method_record.receiver == AwbcTraitReceiverMode::MutRef {
                    let slot = method_record.receiver_state_slot.ok_or_else(|| {
                        VmError::Runtime(
                            "mut trait method is missing receiver state slot".to_owned(),
                        )
                    })?;
                    Some(method_fiber.active_frame()?.register(slot)?.clone())
                } else {
                    None
                };
                return Ok(TraitMethodCallOutcome::Completed(TraitMethodVmOutcome {
                    value,
                    updated_receiver,
                }));
            }
            VmExit::Returned(None) => {
                return Err(VmError::Runtime(
                    "trait method returned unit where a value was required".to_owned(),
                ));
            }
            VmExit::Trapped(trap) => {
                return Err(VmError::Runtime(format!("trait method trapped: {trap:?}")));
            }
            VmExit::Cancelled => {
                return Err(VmError::Runtime(
                    "trait method execution was cancelled".to_owned(),
                ));
            }
            VmExit::Suspended(reason) => {
                return Err(VmError::Runtime(format!(
                    "trait method attempted to suspend: {reason:?}"
                )));
            }
            VmExit::BudgetYield(_) | VmExit::Running => {
                return Err(VmError::Runtime(
                    "trait method did not complete within deterministic call budget".to_owned(),
                ));
            }
        }
    }
}

fn set_record_field_value(
    target: &mut RuntimeValue,
    field: u32,
    value: RuntimeValue,
) -> Result<(), VmError> {
    let identity = crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(field as usize)
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    target
        .replace_record_field(identity, value)
        .map_err(|_| VmError::Runtime(format!("record assignment has no field ordinal {field}")))
}

#[allow(
    clippy::too_many_lines,
    reason = "AWBC terminator dispatch keeps the shared fiber/suspension state machine in one match"
)]
fn execute_terminator(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    _host: &mut impl VmHost,
    context: Option<&VmExecutionContext>,
    terminator: &AwbcTerminator,
    source_map: Option<AwbcSourceMapId>,
    observations: &mut Vec<VmObservation>,
) -> Result<VmExit, VmError> {
    match terminator {
        AwbcTerminator::Jump { target } => {
            jump(fiber, *target);
            Ok(VmExit::Running)
        }
        AwbcTerminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            let condition_value = register(fiber, *condition)?;
            let condition = condition_value.as_bool().ok_or_else(|| {
                VmError::Runtime(format!(
                    "branch condition expected bool, found {}",
                    runtime_value_label(condition_value)
                ))
            })?;
            jump(fiber, if condition { *then_block } else { *else_block });
            Ok(VmExit::Running)
        }
        AwbcTerminator::Match {
            scrutinee,
            arms,
            default,
        } => {
            let value = register(fiber, *scrutinee)?.clone();
            let start = usize::try_from(arms.start)
                .map_err(|_| VmError::Runtime("match arm start does not fit usize".to_owned()))?;
            let end = usize::try_from(arms.checked_end().unwrap_or(arms.start))
                .map_err(|_| VmError::Runtime("match arm end does not fit usize".to_owned()))?;
            let target = program.match_arms[start..end]
                .iter()
                .find_map(|arm| {
                    test_pattern(program, arm.pattern, &value)
                        .ok()
                        .and_then(|matched| matched.then_some(arm.target))
                })
                .unwrap_or(*default);
            jump(fiber, target);
            Ok(VmExit::Running)
        }
        AwbcTerminator::CallFunction {
            function,
            args,
            dst,
            resume,
        } => {
            let args = register_values(fiber, args)?;
            fiber.push_call_frame_with_args(program, *function, *resume, *dst, &args)?;
            Ok(VmExit::Running)
        }
        AwbcTerminator::ProjectCall { call } => execute_project_call(program, fiber, context, call),
        AwbcTerminator::GotoStatic { function, args } => {
            let args = register_values(fiber, args)?;
            emit_unwind_cleanup_observations(fiber, observations);
            fiber.replace_root_function(program, *function, &args)?;
            observations.push(VmObservation::Goto(*function));
            Ok(VmExit::Running)
        }
        AwbcTerminator::GotoDynamic { target, args } => {
            let target_value = register(fiber, *target)?.clone();
            let target = match &target_value {
                RuntimeValue::String(target) => program
                    .resolve_flow_target_value(target)
                    .map(|(_, function)| function)
                    .map_err(|error| VmError::Runtime(error.to_string()))?,
                RuntimeValue::EntityRef(target) => program
                    .resolve_flow_target_value(&target.runtime_label())
                    .map(|(_, function)| function)
                    .map_err(|error| VmError::Runtime(error.to_string()))?,
                _ => {
                    return Err(VmError::Runtime(format!(
                        "invalid dynamic goto target `{}`",
                        runtime_value_label(&target_value)
                    )));
                }
            };
            let args = register_values(fiber, args)?;
            emit_unwind_cleanup_observations(fiber, observations);
            fiber.replace_root_function(program, target, &args)?;
            observations.push(VmObservation::Goto(target));
            Ok(VmExit::Running)
        }
        AwbcTerminator::Dialogue {
            content,
            values,
            effects,
            line_task_captures,
            result,
            resume,
        } => {
            let values = values
                .iter()
                .map(|binding| {
                    Ok(crate::plan::RuntimeDialogueValueBinding {
                        slot: binding.slot,
                        role: match binding.role {
                            crate::awbc::schema::AwbcDialogueValueRole::Interpolation => {
                                crate::plan::RuntimeDialogueValueRole::Interpolation
                            }
                            crate::awbc::schema::AwbcDialogueValueRole::Content => {
                                crate::plan::RuntimeDialogueValueRole::Content
                            }
                        },
                        value: register(fiber, binding.value)?.clone(),
                    })
                })
                .collect::<Result<Vec<_>, VmError>>()?
                .into_boxed_slice();
            let line_task_captures = line_task_captures
                .iter()
                .map(|register_id| register(fiber, *register_id).cloned())
                .collect::<Result<Vec<_>, VmError>>()?
                .into_boxed_slice();
            suspend(
                fiber,
                *resume,
                FiberSuspensionReason::Dialogue {
                    content: *content,
                    values,
                    effects: effects.clone().into_boxed_slice(),
                    line_task_captures,
                    result: result.clone(),
                },
            )
        }
        AwbcTerminator::Choice {
            choice,
            dst,
            resume,
        } => suspend(
            fiber,
            *resume,
            FiberSuspensionReason::Choice {
                choice: *choice,
                destination: *dst,
            },
        ),
        AwbcTerminator::Await {
            handle,
            binding,
            observer,
            resume,
        } => {
            let target = await_target(program, fiber, *handle)?;
            suspend(
                fiber,
                *resume,
                FiberSuspensionReason::Await {
                    target,
                    binding: *binding,
                    observer: *observer,
                },
            )
        }
        AwbcTerminator::AwaitMany {
            plan,
            source,
            binding,
            resume,
        } => {
            let items = match register(fiber, *source)? {
                RuntimeValue::Seq(sequence) => sequence.clone().into_values(),
                value => vec![value.clone()],
            };
            suspend(
                fiber,
                *resume,
                FiberSuspensionReason::AwaitMany(FiberAwaitManyState {
                    plan: *plan,
                    binding: *binding,
                    items,
                    next_index: 0,
                    in_flight: Vec::new(),
                    results: Vec::new(),
                }),
            )
        }
        AwbcTerminator::HostCall {
            call,
            args,
            dst,
            resume,
        } => {
            let args = args
                .iter()
                .map(|arg| register(fiber, *arg).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            suspend(
                fiber,
                *resume,
                FiberSuspensionReason::HostCall {
                    call: *call,
                    args,
                    destination: *dst,
                },
            )
        }
        AwbcTerminator::Return { value } => {
            let value = value
                .map(|value| register(fiber, value).cloned())
                .transpose()?;
            drain_active_frame_cleanups(fiber, observations)?;
            let returning_function = fiber.active_frame()?.function;
            let return_to = fiber.active_frame()?.return_to.clone();
            if fiber.finish_return(program, value.clone())? {
                Ok(VmExit::Returned(value))
            } else {
                if let Some(return_to) = return_to {
                    match return_to.continuation.clone() {
                        FiberReturnContinuation::Ordinary => {}
                        continuation => {
                            complete_project_call_return(
                                program,
                                fiber,
                                returning_function,
                                return_to,
                                continuation,
                                value,
                            )?;
                        }
                    }
                }
                Ok(VmExit::Running)
            }
        }
        AwbcTerminator::Trap { code, message } => {
            let message = message
                .map(|id| string(program, id))
                .transpose()?
                .map(str::to_owned);
            let trap = terminate_with_trap(fiber, *code, message, source_map, observations);
            Ok(VmExit::Trapped(trap))
        }
        AwbcTerminator::BudgetYield { resume } => {
            suspend(fiber, *resume, FiberSuspensionReason::BudgetYield)
        }
        AwbcTerminator::Unreachable => {
            let trap = terminate_with_trap(
                fiber,
                AwbcTrapCode::InternalInvariant,
                Some("unreachable AWBC block executed".to_owned()),
                source_map,
                observations,
            );
            Ok(VmExit::Trapped(trap))
        }
    }
}

fn suspend(
    fiber: &mut FiberState,
    resume: AwbcResumePointId,
    reason: FiberSuspensionReason,
) -> Result<VmExit, VmError> {
    fiber.suspend(FiberSuspension {
        resume: FiberResumeTarget::Declared(resume),
        reason: reason.clone(),
    })?;
    Ok(VmExit::Suspended(reason))
}

/// Executes one closed project-call group against its program-owned callable
/// state. Physical operands are materialized once before the shared callable
/// application helper selects retention, a default, or a body invocation.
fn execute_project_call(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    context: Option<&VmExecutionContext>,
    call: &AwbcProjectCall,
) -> Result<VmExit, VmError> {
    let owner = context
        .ok_or(VmError::MissingExecutionContext)?
        .program_owner(program)?;
    let RuntimeValue::Callable(callable) = register(fiber, call.callee)?.clone() else {
        return Err(VmError::Runtime(
            "project-call callee register does not contain a callable".to_owned(),
        ));
    };
    callable
        .validate_for_owner(&owner)
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    if callable.state() != call.state {
        return Err(VmError::Runtime(
            "project-call callee has a different callable state".to_owned(),
        ));
    }
    let state = program
        .callable_states
        .get(call.state.index())
        .ok_or_else(|| VmError::Runtime("project-call callable state is absent".to_owned()))?;

    let operands = call
        .operands
        .iter()
        .map(|operand| {
            let value = register(fiber, operand.value)?.clone();
            match operand.mode {
                AwbcProjectCallOperandMode::Value => Ok(vec![value]),
                AwbcProjectCallOperandMode::Spread => {
                    crate::value::runtime_value_into_sequence_values(value).map_err(|value| {
                        VmError::Runtime(format!(
                            "project-call spread operand is not a sequence: {}",
                            runtime_value_label(&value)
                        ))
                    })
                }
            }
        })
        .collect::<Result<Vec<_>, VmError>>()?;

    let mut logical_values = Vec::with_capacity(call.ordinary.len());
    for (parameter_index, row) in call.ordinary.iter().enumerate() {
        let parameter = state
            .parameters
            .get(parameter_index)
            .ok_or_else(|| VmError::Runtime("project-call parameter row is absent".to_owned()))?;
        let encoded_parameter = match row {
            AwbcProjectCallOrdinaryMaterialization::Fixed { parameter, .. }
            | AwbcProjectCallOrdinaryMaterialization::Rest { parameter, .. } => *parameter,
        };
        if usize::try_from(encoded_parameter).ok() != Some(parameter_index) {
            return Err(VmError::Runtime(
                "project-call parameter coordinates are not canonical".to_owned(),
            ));
        }
        match row {
            AwbcProjectCallOrdinaryMaterialization::Fixed { source_index, .. } => {
                let value = project_call_source_value(&operands, *source_index)?;
                require_runtime_type(program, parameter.binding_ty, &value)?;
                logical_values.push(value);
            }
            AwbcProjectCallOrdinaryMaterialization::Rest { source_indices, .. } => {
                let mut values = Vec::new();
                for source_index in source_indices {
                    let source = operands
                        .get(usize::try_from(*source_index).map_err(|_| {
                            VmError::Runtime("project-call source index exceeds usize".to_owned())
                        })?)
                        .ok_or_else(|| {
                            VmError::Runtime("project-call source is absent".to_owned())
                        })?;
                    for value in source {
                        require_runtime_type(program, parameter.abi_ty, value)?;
                        values.push(value.clone());
                    }
                }
                let packed = RuntimeValue::Seq(RuntimeSeq::Values(values));
                require_runtime_type(program, parameter.binding_ty, &packed)?;
                logical_values.push(packed);
            }
        }
    }
    let attached = match &call.attached {
        None => None,
        Some(attached) => match &attached.presence {
            AwbcProjectCallAttachedPresence::RequiredPresent
            | AwbcProjectCallAttachedPresence::OptionalPresent
            | AwbcProjectCallAttachedPresence::DefaultedPresent => {
                let source_index = attached.source_index.ok_or_else(|| {
                    VmError::Runtime("present attached project-call source is absent".to_owned())
                })?;
                Some(project_call_source_value(&operands, source_index)?)
            }
            AwbcProjectCallAttachedPresence::OptionalOmitted
            | AwbcProjectCallAttachedPresence::DefaultedOmitted => None,
        },
    };
    let application = callable
        .prepare_group(&logical_values, attached)
        .map_err(|error| VmError::Runtime(error.to_string()))?;

    let resume = program
        .resume_points
        .get(call.resume.index())
        .ok_or(VmError::Fiber(FiberStateError::UnknownResumePoint(
            call.resume.0,
        )))?;
    if resume.function != fiber.cursor.function {
        return Err(VmError::Runtime(
            "project-call resume point belongs to another function".to_owned(),
        ));
    }
    let site = AwbcProjectCallSite {
        caller_function: fiber.cursor.function,
        block: fiber.cursor.block,
    };
    match application {
        RuntimeCallableApplication::Complete(value) => {
            require_runtime_type(program, state.result, &value)?;
            bind_pattern(program, fiber, call.result_pattern, &value)?;
            jump(fiber, resume.block);
        }
        RuntimeCallableApplication::Invoke(invocation) => {
            let function = invocation_function(&invocation)?;
            let args = invocation_values(invocation)?;
            let point = project_call_return_point(program, fiber, call.resume, site)?;
            let return_to = FiberReturnPoint {
                continuation: FiberReturnContinuation::ProjectCallTarget { site },
                ..point
            };
            fiber.push_call_frame_with_continuation(program, function, return_to, &args)?;
        }
        RuntimeCallableApplication::AttachedDefault(invocation) => {
            let function = invocation_function(&invocation)?;
            let args = invocation_values(invocation)?;
            let point = project_call_return_point(program, fiber, call.resume, site)?;
            let return_to = FiberReturnPoint {
                continuation: FiberReturnContinuation::ProjectCallDefault {
                    site,
                    logical_values,
                },
                ..point
            };
            fiber.push_call_frame_with_continuation(program, function, return_to, &args)?;
        }
    }
    Ok(VmExit::Running)
}

fn project_call_return_point(
    program: &AwbcProgram,
    fiber: &FiberState,
    resume: AwbcResumePointId,
    site: AwbcProjectCallSite,
) -> Result<FiberReturnPoint, VmError> {
    let caller = fiber.active_frame()?;
    let point = program
        .resume_points
        .get(resume.index())
        .ok_or(VmError::Fiber(FiberStateError::UnknownResumePoint(
            resume.0,
        )))?;
    if site.caller_function != caller.function
        || point.function != caller.function
        || point.frame_layout != caller.layout
    {
        return Err(VmError::Runtime(
            "project-call return site does not match its caller".to_owned(),
        ));
    }
    Ok(FiberReturnPoint {
        cursor: FiberCursor {
            function: point.function,
            block: point.block,
            instruction_offset: 0,
        },
        destination: None,
        continuation: FiberReturnContinuation::Ordinary,
    })
}

fn project_call_at_site<'a>(
    program: &'a AwbcProgram,
    site: AwbcProjectCallSite,
) -> Result<&'a AwbcProjectCall, VmError> {
    let block = program
        .blocks
        .get(site.block.index())
        .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
    if block.owner != site.caller_function {
        return Err(VmError::Fiber(FiberStateError::InvalidFrame));
    }
    let AwbcTerminator::ProjectCall { call } = &block.terminator else {
        return Err(VmError::Fiber(FiberStateError::InvalidFrame));
    };
    Ok(call)
}

fn complete_project_call_return(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    returning_function: AwbcFunctionId,
    return_to: FiberReturnPoint,
    continuation: FiberReturnContinuation,
    value: Option<RuntimeValue>,
) -> Result<(), VmError> {
    let continuation = match continuation {
        FiberReturnContinuation::ApplyGroupDefault {
            callable,
            arguments,
            destination,
        } => {
            let RuntimeValue::Callable(callable) = callable else {
                return Err(VmError::Runtime(
                    "callable default continuation lost its callable".to_owned(),
                ));
            };
            let default_value = value.ok_or_else(|| {
                VmError::Runtime("callable default returned no attached value".to_owned())
            })?;
            let application = callable
                .complete_group_default(&arguments, default_value)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            match application {
                RuntimeCallableApplication::Complete(value) => {
                    fiber.active_frame_mut()?.set_register(destination, value)?;
                }
                RuntimeCallableApplication::Invoke(invocation) => {
                    let function = invocation_function(&invocation)?;
                    let args = invocation_values(invocation)?;
                    let return_to = FiberReturnPoint::ordinary(return_to.cursor, Some(destination));
                    fiber.push_call_frame_with_continuation(program, function, return_to, &args)?;
                }
                RuntimeCallableApplication::AttachedDefault(_) => {
                    return Err(VmError::Runtime(
                        "callable default selected another default stage".to_owned(),
                    ));
                }
            }
            return Ok(());
        }
        other => other,
    };
    let site = match &continuation {
        FiberReturnContinuation::ProjectCallDefault { site, .. }
        | FiberReturnContinuation::ProjectCallTarget { site } => *site,
        FiberReturnContinuation::ApplyGroupDefault { .. } => unreachable!(),
        FiberReturnContinuation::Ordinary => return Ok(()),
    };
    let call = project_call_at_site(program, site)?;
    let resume = program
        .resume_points
        .get(call.resume.index())
        .ok_or(VmError::Fiber(FiberStateError::UnknownResumePoint(
            call.resume.0,
        )))?;
    if return_to.cursor
        != (FiberCursor {
            function: resume.function,
            block: resume.block,
            instruction_offset: 0,
        })
    {
        return Err(VmError::Runtime(
            "project-call return cursor does not match its verified site".to_owned(),
        ));
    }
    match continuation {
        FiberReturnContinuation::ProjectCallDefault { logical_values, .. } => {
            let default_value = value.ok_or_else(|| {
                VmError::Runtime("project-call default returned no attached value".to_owned())
            })?;
            let RuntimeValue::Callable(callable) = register(fiber, call.callee)?.clone() else {
                return Err(VmError::Runtime(
                    "project-call default lost its callable".to_owned(),
                ));
            };
            if callable.state() != call.state {
                return Err(VmError::Runtime(
                    "project-call default resumed with a different callable state".to_owned(),
                ));
            }
            let state = program
                .callable_states
                .get(call.state.index())
                .ok_or_else(|| VmError::Runtime("callable state is absent".to_owned()))?;
            let crate::plan::RuntimeCallableAttachedContract::Defaulted { default, .. } =
                &state.attached
            else {
                return Err(VmError::Runtime(
                    "project-call default returned into a state without a default".to_owned(),
                ));
            };
            if returning_function != default.function {
                return Err(VmError::Runtime(
                    "project-call returned from an unexpected default function".to_owned(),
                ));
            }
            let application = callable
                .complete_group_default(&logical_values, default_value)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            match application {
                RuntimeCallableApplication::Complete(result) => {
                    require_runtime_type(program, state.result, &result)?;
                    bind_pattern(program, fiber, call.result_pattern, &result)?;
                    jump(fiber, resume.block);
                }
                RuntimeCallableApplication::Invoke(invocation) => {
                    let function = invocation_function(&invocation)?;
                    let args = invocation_values(invocation)?;
                    let point = project_call_return_point(program, fiber, call.resume, site)?;
                    let return_to = FiberReturnPoint {
                        continuation: FiberReturnContinuation::ProjectCallTarget { site },
                        ..point
                    };
                    fiber.push_call_frame_with_continuation(program, function, return_to, &args)?;
                }
                RuntimeCallableApplication::AttachedDefault(_) => {
                    return Err(VmError::Runtime(
                        "project-call default selected another default stage".to_owned(),
                    ));
                }
            }
        }
        FiberReturnContinuation::ProjectCallTarget { .. } => {
            let Some(state) = program.callable_states.get(call.state.index()) else {
                return Err(VmError::Runtime(
                    "project-call target state is absent".to_owned(),
                ));
            };
            let crate::plan::RuntimeCallableTransition::Invoke { function, .. } = &state.transition
            else {
                return Err(VmError::Runtime(
                    "project-call target state does not invoke".to_owned(),
                ));
            };
            if returning_function != *function {
                return Err(VmError::Runtime(
                    "project-call returned from an unexpected target function".to_owned(),
                ));
            }
            let value = value.ok_or_else(|| {
                VmError::Runtime("project-call target returned no result".to_owned())
            })?;
            require_runtime_type(program, state.result, &value)?;
            bind_pattern(program, fiber, call.result_pattern, &value)?;
            jump(fiber, resume.block);
        }
        FiberReturnContinuation::ApplyGroupDefault { .. } => unreachable!(
            "apply-group default return is handled before looking up a project-call site"
        ),
        FiberReturnContinuation::Ordinary => {}
    }
    Ok(())
}

fn project_call_source_value(
    operands: &[Vec<RuntimeValue>],
    index: u32,
) -> Result<RuntimeValue, VmError> {
    let source =
        operands
            .get(usize::try_from(index).map_err(|_| {
                VmError::Runtime("project-call source index exceeds usize".to_owned())
            })?)
            .ok_or_else(|| VmError::Runtime("project-call source is absent".to_owned()))?;
    if source.len() != 1 {
        return Err(VmError::Runtime(
            "fixed project-call source expanded to multiple values".to_owned(),
        ));
    }
    Ok(source[0].clone())
}

fn require_runtime_type(
    program: &AwbcProgram,
    expected: AwbcTypeId,
    value: &RuntimeValue,
) -> Result<(), VmError> {
    if runtime_value_matches_type(program, value, expected, 0) {
        Ok(())
    } else {
        Err(VmError::Runtime(format!(
            "project-call value {} does not match runtime type {}",
            runtime_value_label(value),
            expected.0
        )))
    }
}

fn register(fiber: &FiberState, register: AwbcRegisterId) -> Result<&RuntimeValue, VmError> {
    fiber
        .active_frame()?
        .register(register)
        .map_err(VmError::from)
}

fn materialize_drop_policy(
    fiber: &FiberState,
    policy: AwbcDropPolicy,
) -> Result<crate::effect::RuntimeDropPolicy, VmError> {
    use crate::effect::RuntimeDropPolicy;

    Ok(match policy {
        AwbcDropPolicy::Default => RuntimeDropPolicy::Default,
        AwbcDropPolicy::Cancel => RuntimeDropPolicy::Cancel,
        AwbcDropPolicy::Stop { fade } => {
            let RuntimeValue::Duration(fade) = register(fiber, fade)? else {
                return Err(VmError::Runtime(
                    "Drop Stop fade register does not contain Duration".to_owned(),
                ));
            };
            RuntimeDropPolicy::Stop { fade: *fade }
        }
        AwbcDropPolicy::Finish => RuntimeDropPolicy::Finish,
        AwbcDropPolicy::Release => RuntimeDropPolicy::Release,
        AwbcDropPolicy::Detach => RuntimeDropPolicy::Detach,
    })
}

fn await_target(
    program: &AwbcProgram,
    fiber: &FiberState,
    register_id: AwbcRegisterId,
) -> Result<FiberAwaitTarget, VmError> {
    let frame = fiber.active_frame()?;
    let runtime_type = program
        .frame_layouts
        .get(frame.layout.index())
        .and_then(|layout| layout.slots.get(register_id.index()))
        .and_then(|slot| program.runtime_types.get(slot.ty.index()))
        .ok_or_else(|| VmError::Runtime("await handle register has no runtime type".to_owned()))?;
    let value = register(fiber, register_id)?.clone();
    match runtime_type.shape() {
        AwbcRuntimeTypeShape::Need(_) => match value {
            RuntimeValue::String(need) if !need.is_empty() => {
                Ok(FiberAwaitTarget::Need(NeedId(need)))
            }
            value => Err(VmError::Runtime(format!(
                "NeedHandle register contained {}",
                runtime_value_label(&value)
            ))),
        },
        AwbcRuntimeTypeShape::Task(_) | AwbcRuntimeTypeShape::Dynamic => {
            Ok(FiberAwaitTarget::Task(value))
        }
        _ => Err(VmError::Runtime(
            "await register is neither a task handle nor a Need handle".to_owned(),
        )),
    }
}

fn register_values(
    fiber: &FiberState,
    registers: &[AwbcRegisterId],
) -> Result<Vec<RuntimeValue>, VmError> {
    registers
        .iter()
        .map(|register_id| register(fiber, *register_id).cloned())
        .collect()
}

fn jump(fiber: &mut FiberState, block: AwbcBlockId) {
    fiber.cursor.block = block;
    fiber.cursor.instruction_offset = 0;
}

fn terminal_exit(fiber: &FiberState) -> VmExit {
    match fiber.terminal.as_ref() {
        Some(FiberTerminalValue::Returned(value)) => VmExit::Returned(value.clone()),
        Some(FiberTerminalValue::Cancelled) => VmExit::Cancelled,
        Some(FiberTerminalValue::Trapped(trap)) => VmExit::Trapped(trap.clone()),
        None if matches!(fiber.status, FiberStatus::Suspended) => fiber
            .suspension
            .as_ref()
            .map_or(VmExit::Running, |suspension| {
                VmExit::Suspended(suspension.reason.clone())
            }),
        None => VmExit::Running,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "constant materialization exhaustively mirrors the closed AWBC constant family"
)]
impl AwbcProgram {
    fn make_record_value(
        &self,
        ty: AwbcTypeId,
        values: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, VmError> {
        let row = self
            .runtime_types
            .get(ty.index())
            .ok_or(VmError::MissingType(ty))?;
        match row.shape() {
            AwbcRuntimeTypeShape::NominalRecord { .. } => {
                let layout = self
                    .nominal_record_layout(ty)
                    .map_err(|error| VmError::Runtime(error.to_string()))?
                    .expect("nominal-record row supplies an executable layout");
                RuntimeNominalRecordValue::try_from_accepted_layout(&layout, values)
                    .map(RuntimeValue::NominalRecord)
                    .map_err(|error| VmError::Runtime(error.to_string()))
            }
            AwbcRuntimeTypeShape::Record { fields, .. } => {
                self.validate_record_fields(
                    ty,
                    crate::entry::RuntimeNominalRecordShape::Record,
                    fields,
                )
                .map_err(|error| VmError::Runtime(error.to_string()))?;
                if fields.len() != values.len() {
                    return Err(VmError::Runtime(
                        "record value field count does not match its type".to_owned(),
                    ));
                }
                let values = fields
                    .iter()
                    .zip(values)
                    .map(|(field, value)| {
                        let name = field
                            .name
                            .expect("record source shape requires field names");
                        RuntimeFieldValue::new_accepted(
                            field.field,
                            self.strings[name.index()].clone(),
                            value,
                        )
                    })
                    .collect();
                RuntimeRecordValue::try_from_fields(values)
                    .map(RuntimeValue::Record)
                    .map_err(|error| VmError::Runtime(error.to_string()))
            }
            _ => Err(VmError::Runtime(
                "record construction references a non-record type".to_owned(),
            )),
        }
    }
}

pub(crate) fn constant_value(
    program: &AwbcProgram,
    constant: AwbcConstantId,
) -> Result<RuntimeValue, VmError> {
    let constant = program
        .constants
        .get(constant.index())
        .ok_or(VmError::MissingConstant(constant))?;
    match constant {
        AwbcConstant::Unit => Ok(RuntimeValue::Unit),
        AwbcConstant::Bool(value) => Ok(RuntimeValue::Bool(*value)),
        AwbcConstant::Int { kind, bits } => signed_value(*kind, *bits),
        AwbcConstant::UInt { kind, bits } => unsigned_value(*kind, *bits),
        AwbcConstant::F32Bits(bits) => Ok(RuntimeValue::F32(f32::from_bits(*bits))),
        AwbcConstant::F64Bits(bits) => Ok(RuntimeValue::F64(f64::from_bits(*bits))),
        AwbcConstant::String(id) => Ok(RuntimeValue::String(string(program, *id)?.to_owned())),
        AwbcConstant::Char(value) => char::from_u32(*value)
            .map(RuntimeValue::Char)
            .ok_or_else(|| VmError::Runtime(format!("invalid char scalar {value}"))),
        AwbcConstant::DurationNanos(nanos) => {
            Ok(RuntimeValue::Duration(LogicalDuration::from_nanos(*nanos)))
        }
        AwbcConstant::EntityRef(value) => Ok(RuntimeValue::EntityRef(value.clone())),
        AwbcConstant::Tuple(items) => Ok(RuntimeValue::Tuple(
            items
                .iter()
                .map(|item| constant_value(program, *item))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        AwbcConstant::Sequence(items) => Ok(runtime_sequence_from_literal_values(
            items
                .iter()
                .map(|item| constant_value(program, *item))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        AwbcConstant::Record { ty, fields } => {
            let values = fields
                .iter()
                .map(|field| constant_value(program, *field))
                .collect::<Result<Vec<_>, VmError>>()?;
            program.make_record_value(*ty, values)
        }
        AwbcConstant::Variant { ty, case, payload } => {
            let Some(AwbcRuntimeTypeShape::Variant { cases, .. }) = program
                .runtime_types
                .get(ty.index())
                .map(AwbcRuntimeType::shape)
            else {
                return Err(VmError::Runtime(
                    "variant constant references a non-variant type".to_owned(),
                ));
            };
            let selected = cases.get(*case as usize).ok_or_else(|| {
                VmError::Runtime("variant constant case is out of bounds".to_owned())
            })?;
            Ok(RuntimeValue::Variant {
                owner: variant_identity_for_type(program, *ty)?,
                ordinal: *case,
                name: string(program, selected.name)?.to_owned(),
                payload: payload
                    .map(|id| constant_value(program, id))
                    .transpose()?
                    .map(Box::new),
            })
        }
        AwbcConstant::Opaque { ty, payload } => {
            let owner = program
                .opaque_owner(*ty)
                .map_err(|error| VmError::Runtime(error.to_string()))?
                .ok_or_else(|| {
                    VmError::Runtime("opaque constant references a non-opaque type".to_owned())
                })?;
            if owner.persistence() == crate::value::RuntimeOpaquePersistence::SnapshotOnly
                || matches!(
                    owner.value_class(),
                    crate::value::RuntimeOpaqueValueClass::AffineHandle(_)
                )
            {
                return Err(VmError::Runtime(
                    "non-constant opaque type cannot materialize from a constant".to_owned(),
                ));
            }
            let payload = constant_value(program, *payload)?;
            owner
                .try_wrap(payload)
                .map_err(|error| VmError::Runtime(error.to_string()))
        }
        AwbcConstant::Range {
            start,
            end,
            inclusive,
        } => {
            let start = start.map(|id| constant_value(program, id)).transpose()?;
            let end = end.map(|id| constant_value(program, id)).transpose()?;
            crate::value::RuntimeRange::new(start, end, *inclusive)
                .map(RuntimeValue::Range)
                .map_err(|error| VmError::Runtime(error.to_string()))
        }
        AwbcConstant::Bytes(bytes) => Ok(RuntimeValue::Seq(RuntimeSeq::dense_bytes(bytes.clone()))),
        AwbcConstant::TensorF32 { shape, values } => crate::math::DenseTensorF32::new(
            shape_to_usize_vec(shape)?,
            values.iter().map(|value| f32::from_bits(*value)).collect(),
        )
        .map(RuntimeValue::TensorF32)
        .map_err(|error| VmError::Runtime(error.to_string())),
        AwbcConstant::TensorF64 { shape, values } => crate::math::DenseTensorF64::new(
            shape_to_usize_vec(shape)?,
            values.iter().map(|value| f64::from_bits(*value)).collect(),
        )
        .map(RuntimeValue::TensorF64)
        .map_err(|error| VmError::Runtime(error.to_string())),
    }
}

fn shape_to_usize_vec(shape: &[u32]) -> Result<Vec<usize>, VmError> {
    shape
        .iter()
        .map(|value| {
            usize::try_from(*value)
                .map_err(|_| VmError::Runtime("tensor shape does not fit usize".to_owned()))
        })
        .collect()
}

fn signed_value(kind: AwbcSignedIntKind, bits: [u8; 16]) -> Result<RuntimeValue, VmError> {
    let value = i128::from_le_bytes(bits);
    Ok(match kind {
        AwbcSignedIntKind::I8 => RuntimeValue::i8(
            i8::try_from(value).map_err(|_| VmError::Runtime("i8 constant overflow".to_owned()))?,
        ),
        AwbcSignedIntKind::I16 => RuntimeValue::i16(
            i16::try_from(value)
                .map_err(|_| VmError::Runtime("i16 constant overflow".to_owned()))?,
        ),
        AwbcSignedIntKind::I32 => RuntimeValue::i32(
            i32::try_from(value)
                .map_err(|_| VmError::Runtime("i32 constant overflow".to_owned()))?,
        ),
        AwbcSignedIntKind::I64 => RuntimeValue::i64(
            i64::try_from(value)
                .map_err(|_| VmError::Runtime("i64 constant overflow".to_owned()))?,
        ),
        AwbcSignedIntKind::I128 => RuntimeValue::i128(value),
        AwbcSignedIntKind::ISize => RuntimeValue::isize(
            i64::try_from(value)
                .map_err(|_| VmError::Runtime("isize constant overflow".to_owned()))?,
        ),
    })
}

fn unsigned_value(kind: AwbcUnsignedIntKind, bits: [u8; 16]) -> Result<RuntimeValue, VmError> {
    let value = u128::from_le_bytes(bits);
    Ok(match kind {
        AwbcUnsignedIntKind::U8 => RuntimeValue::u8(
            u8::try_from(value).map_err(|_| VmError::Runtime("u8 constant overflow".to_owned()))?,
        ),
        AwbcUnsignedIntKind::U16 => RuntimeValue::u16(
            u16::try_from(value)
                .map_err(|_| VmError::Runtime("u16 constant overflow".to_owned()))?,
        ),
        AwbcUnsignedIntKind::U32 => RuntimeValue::u32(
            u32::try_from(value)
                .map_err(|_| VmError::Runtime("u32 constant overflow".to_owned()))?,
        ),
        AwbcUnsignedIntKind::U64 => RuntimeValue::u64(
            u64::try_from(value)
                .map_err(|_| VmError::Runtime("u64 constant overflow".to_owned()))?,
        ),
        AwbcUnsignedIntKind::U128 => RuntimeValue::u128(value),
        AwbcUnsignedIntKind::USize => RuntimeValue::usize(
            u64::try_from(value)
                .map_err(|_| VmError::Runtime("usize constant overflow".to_owned()))?,
        ),
    })
}

fn string(program: &AwbcProgram, id: AwbcStringId) -> Result<&str, VmError> {
    program
        .strings
        .get(id.index())
        .map(String::as_str)
        .ok_or(VmError::MissingString(id))
}

fn variant_identity_for_type(
    program: &AwbcProgram,
    ty: AwbcTypeId,
) -> Result<crate::pattern::RuntimeVariantIdentity, VmError> {
    match program.runtime_types.get(ty.index()) {
        Some(runtime_type) => match runtime_type.shape() {
            AwbcRuntimeTypeShape::Variant { owner, .. } => {
                runtime_variant_identity(program, runtime_type.semantic_identity(), owner)
                    .ok_or_else(|| VmError::Runtime("variant owner identity is invalid".to_owned()))
            }
            _ => Err(VmError::Runtime(
                "variant value references a non-variant runtime type".to_owned(),
            )),
        },
        None => Err(VmError::MissingType(ty)),
    }
}

pub(crate) fn test_pattern(
    program: &AwbcProgram,
    pattern: AwbcPatternId,
    value: &RuntimeValue,
) -> Result<bool, VmError> {
    let pattern = program
        .patterns
        .get(pattern.index())
        .ok_or(VmError::MissingPattern(pattern))?;
    Ok(match pattern {
        AwbcPattern::Bind { expected, .. } => {
            expected.is_none_or(|expected| runtime_value_matches_type(program, value, expected, 0))
        }
        AwbcPattern::Discard => true,
        AwbcPattern::Literal(id) => constant_value(program, *id)? == *value,
        AwbcPattern::Entity(expected) => {
            matches!(value, RuntimeValue::EntityRef(actual) if actual == expected)
        }
        AwbcPattern::Tuple(patterns) => {
            matches!(value, RuntimeValue::Tuple(values) if values.len() == patterns.len() && patterns.iter().zip(values).all(|(pattern, value)| test_pattern(program, *pattern, value).unwrap_or(false)))
        }
        AwbcPattern::Record { ty, fields, rest } => {
            let owner_matches =
                ty.is_none_or(|ty| runtime_value_matches_type(program, value, ty, 0));
            owner_matches
                && match value {
                    RuntimeValue::Record(values) => {
                        rest.accepts_len(fields.len(), values.len())
                            && fields.iter().all(|field| {
                                values.get(field.field as usize).is_some_and(|value| {
                                    test_pattern(program, field.pattern, value.value())
                                        .unwrap_or(false)
                                })
                            })
                    }
                    RuntimeValue::NominalRecord(record) => {
                        rest.accepts_len(fields.len(), record.fields().len())
                            && fields.iter().all(|field| {
                                record
                                    .fields()
                                    .get(field.field as usize)
                                    .is_some_and(|value| {
                                        test_pattern(program, field.pattern, value).unwrap_or(false)
                                    })
                            })
                    }
                    _ => false,
                }
        }
        AwbcPattern::Sequence { items, rest } => {
            matches!(value, RuntimeValue::Seq(sequence) if rest.accepts_len(items.len(), sequence.len()) && items.iter().enumerate().all(|(index, pattern)| test_pattern(program, *pattern, &sequence.value_at(index)).unwrap_or(false)))
        }
        AwbcPattern::Variant {
            ty,
            case,
            case_name,
            payload,
        } => {
            let case_name = string(program, *case_name)?;
            runtime_value_matches_type(program, value, *ty, 0)
                && matches!(value, RuntimeValue::Variant { ordinal, name, payload: actual, .. } if case == ordinal && case_name == name && payload.is_none_or(|pattern| actual.as_deref().is_some_and(|value| test_pattern(program, pattern, value).unwrap_or(false))))
        }
        AwbcPattern::Whole { inner, .. } => test_pattern(program, *inner, value)?,
    })
}

pub(crate) fn bind_pattern(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    pattern: AwbcPatternId,
    value: &RuntimeValue,
) -> Result<(), VmError> {
    if !test_pattern(program, pattern, value)? {
        return Err(VmError::Runtime("pattern did not match".to_owned()));
    }
    bind_tested_pattern(program, fiber, pattern, value)
}

/// Applies a pattern graph only after the complete root has matched.
///
/// Keeping all writes behind the root pretest makes binding atomic with
/// respect to ordinary mismatch: no child register can be written before a
/// later exact-length, literal, or type predicate fails.
fn bind_tested_pattern(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    pattern: AwbcPatternId,
    value: &RuntimeValue,
) -> Result<(), VmError> {
    let pattern_record = program
        .patterns
        .get(pattern.index())
        .ok_or(VmError::MissingPattern(pattern))?
        .clone();
    match pattern_record {
        AwbcPattern::Bind { target, .. } => {
            fiber
                .active_frame_mut()?
                .set_register(target, value.clone())?;
        }
        AwbcPattern::Whole { target, inner } => {
            bind_tested_pattern(program, fiber, inner, value)?;
            fiber
                .active_frame_mut()?
                .set_register(target, value.clone())?;
        }
        AwbcPattern::Tuple(children) => {
            if let RuntimeValue::Tuple(values) = value {
                for (child, value) in children.into_iter().zip(values) {
                    bind_tested_pattern(program, fiber, child, value)?;
                }
            }
        }
        AwbcPattern::Sequence { items, rest } => {
            if let RuntimeValue::Seq(sequence) = value {
                let item_count = items.len();
                for (index, child) in items.iter().copied().enumerate() {
                    bind_tested_pattern(program, fiber, child, &sequence.value_at(index))?;
                }
                if let AwbcPatternRest::Bind(rest) = rest {
                    fiber
                        .active_frame_mut()?
                        .set_register(rest, RuntimeValue::Seq(sequence.tail_from(item_count)))?;
                }
            }
        }
        AwbcPattern::Record { fields, rest, .. } => {
            match value {
                RuntimeValue::Record(values) => {
                    for field in fields {
                        let value = values.get(field.field as usize).ok_or_else(|| {
                            VmError::Runtime("record pattern field is absent".to_owned())
                        })?;
                        bind_tested_pattern(program, fiber, field.pattern, value.value())?;
                    }
                }
                RuntimeValue::NominalRecord(record) => {
                    for field in fields {
                        let value = record.fields().get(field.field as usize).ok_or_else(|| {
                            VmError::Runtime("record pattern field is absent".to_owned())
                        })?;
                        bind_tested_pattern(program, fiber, field.pattern, value)?;
                    }
                }
                _ => unreachable!("record pattern was tested before binding"),
            }
            if let AwbcPatternRest::Bind(rest) = rest {
                fiber
                    .active_frame_mut()?
                    .set_register(rest, value.clone())?;
            }
        }
        AwbcPattern::Variant { payload, .. } => {
            if let (
                Some(pattern),
                RuntimeValue::Variant {
                    payload: Some(value),
                    ..
                },
            ) = (payload, value)
            {
                bind_tested_pattern(program, fiber, pattern, value)?;
            }
        }
        AwbcPattern::Discard | AwbcPattern::Literal(_) | AwbcPattern::Entity(_) => {}
    }
    Ok(())
}

fn trap(
    fiber: &mut FiberState,
    code: AwbcTrapCode,
    message: Option<&str>,
    source_map: Option<AwbcSourceMapId>,
    observations: &mut Vec<VmObservation>,
) {
    terminate_with_trap(
        fiber,
        code,
        message.map(str::to_owned),
        source_map,
        observations,
    );
}

fn mark_runtime_error_trap(
    fiber: &mut FiberState,
    code: AwbcTrapCode,
    message: String,
    source_map: Option<AwbcSourceMapId>,
    observations: &mut Vec<VmObservation>,
) -> FiberTrap {
    terminate_with_trap(fiber, code, Some(message), source_map, observations)
}

fn terminate_with_trap(
    fiber: &mut FiberState,
    code: AwbcTrapCode,
    message: Option<String>,
    source_map: Option<AwbcSourceMapId>,
    observations: &mut Vec<VmObservation>,
) -> FiberTrap {
    emit_unwind_cleanup_observations(fiber, observations);
    let trap = FiberTrap {
        code,
        message,
        source_map,
    };
    observations.push(VmObservation::Trap(trap.clone()));
    fiber.mark_trapped(trap.clone());
    trap
}

fn source_map_for_location(
    program: &AwbcProgram,
    location: AwbcCodeLocation,
) -> Option<AwbcSourceMapId> {
    program
        .source_map
        .iter()
        .position(|entry| entry.location == location)
        .and_then(|index| u32::try_from(index).ok())
        .map(AwbcSourceMapId)
}

impl VmError {
    fn runtime_trap_code(&self) -> Option<AwbcTrapCode> {
        match self {
            Self::Runtime(message) => Some(if message.contains("division by zero") {
                AwbcTrapCode::DivisionByZero
            } else if message.contains("pattern") {
                AwbcTrapCode::PatternMismatch
            } else if message.contains("dynamic goto target") {
                AwbcTrapCode::MissingDynamicTarget
            } else if message.contains("expected") || message.contains("type") {
                AwbcTrapCode::TypeMismatch
            } else {
                AwbcTrapCode::InternalInvariant
            }),
            Self::Fiber(FiberStateError::RegisterOutOfBounds { .. }) => {
                Some(AwbcTrapCode::UninitializedRegister)
            }
            Self::Fiber(
                FiberStateError::ReturnValueMismatch | FiberStateError::ArgumentType { .. },
            )
            | Self::FunctionArgumentCount { .. } => Some(AwbcTrapCode::TypeMismatch),
            Self::MissingIntrinsic(_) => Some(AwbcTrapCode::HostAbiMismatch),
            Self::MissingExecutionContext | Self::Fiber(_) => Some(AwbcTrapCode::InternalInvariant),
            Self::MissingFunction(_)
            | Self::MissingBlock(_)
            | Self::MissingInstruction(_)
            | Self::MissingConstant(_)
            | Self::MissingString(_)
            | Self::MissingPattern(_)
            | Self::MissingType(_)
            | Self::MissingTraitMethod(_) => None,
        }
    }
}

fn unary_op(op: AwbcUnaryOp) -> crate::value::RuntimeUnaryOp {
    match op {
        AwbcUnaryOp::Not => crate::value::RuntimeUnaryOp::Not,
        AwbcUnaryOp::Neg => crate::value::RuntimeUnaryOp::Neg,
    }
}

fn binary_op(op: AwbcBinaryOp) -> crate::value::RuntimeBinaryOp {
    match op {
        AwbcBinaryOp::Eq => crate::value::RuntimeBinaryOp::Eq,
        AwbcBinaryOp::Ne => crate::value::RuntimeBinaryOp::Ne,
        AwbcBinaryOp::Lt => crate::value::RuntimeBinaryOp::Lt,
        AwbcBinaryOp::Le => crate::value::RuntimeBinaryOp::Le,
        AwbcBinaryOp::Gt => crate::value::RuntimeBinaryOp::Gt,
        AwbcBinaryOp::Ge => crate::value::RuntimeBinaryOp::Ge,
        AwbcBinaryOp::Add => crate::value::RuntimeBinaryOp::Add,
        AwbcBinaryOp::Sub => crate::value::RuntimeBinaryOp::Sub,
        AwbcBinaryOp::Mul => crate::value::RuntimeBinaryOp::Mul,
        AwbcBinaryOp::Div => crate::value::RuntimeBinaryOp::Div,
        AwbcBinaryOp::And => crate::value::RuntimeBinaryOp::And,
        AwbcBinaryOp::Or => crate::value::RuntimeBinaryOp::Or,
    }
}
