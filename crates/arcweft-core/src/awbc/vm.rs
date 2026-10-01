//! Compact AWBC VM executor.
//!
//! The VM is Sans I/O. Host operations, line tasks, dialogue, choice, await,
//! await-many and budget yields are surfaced as typed exits over `FiberState`.
//! This module never falls back to the structured VM.

use super::fiber::{
    AwbcProjectCallSite, FiberAwaitManyState, FiberAwaitTarget, FiberCursor, FiberFrame,
    FiberResumeTarget, FiberReturnContinuation, FiberReturnPoint, FiberSafePoint,
    FiberScopeCleanup, FiberState, FiberStateError, FiberStatus, FiberSuspension,
    FiberSuspensionReason, FiberTerminalValue, FiberTrap, runtime_value_matches_type,
    runtime_variant_identity,
};
use super::schema::{
    AwbcBinaryOp, AwbcBlockId, AwbcCodeLocation, AwbcConstant, AwbcConstantId, AwbcContentUnitId,
    AwbcDropPolicy, AwbcEffectPlanId, AwbcFieldProjection, AwbcFunctionId, AwbcHostCallId,
    AwbcInstruction, AwbcInstructionId, AwbcIntrinsicId, AwbcLineOperation, AwbcLineOperationId,
    AwbcMutablePlace, AwbcOpcode, AwbcPattern, AwbcPatternId, AwbcPatternRest, AwbcProgram,
    AwbcProjectCall, AwbcProjectCallAttachedPresence, AwbcProjectCallOperandMode,
    AwbcProjectCallOrdinaryMaterialization, AwbcPureHelperId, AwbcRegisterId, AwbcResumePointId,
    AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcSignedIntKind, AwbcSourceMapId, AwbcStreamPlanId,
    AwbcStringId, AwbcTaskPlanId, AwbcTerminator, AwbcTraitMethodId, AwbcTrapCode, AwbcTypeId,
    AwbcUnaryOp, AwbcUnsignedIntKind,
};
use crate::effect::RuntimeArtifactFingerprint;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableRetainedRole, RuntimeCallableTransition,
    RuntimeFlowTargetError, RuntimeFunctionInputOwnershipRequirement,
};
use crate::stream::RuntimeStreamYieldCopyProof;
use crate::task::RuntimeProgramOwner;
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeAgentValue, RuntimeArcError, RuntimeArcErrorContextKind, RuntimeArcErrorContextPending,
    RuntimeArcErrorContextStart, RuntimeArcErrorFrame, RuntimeCallableApplication,
    RuntimeCallableBodyReference, RuntimeCallableInvocation, RuntimeCallableMaterializedArgument,
    RuntimeCallableValue, RuntimeDialogueContentValue, RuntimeFieldValue,
    RuntimeNominalRecordValue, RuntimeRecordFieldId, RuntimeRecordValue, RuntimeReductionValue,
    RuntimeScalarView, RuntimeSeq, RuntimeValue, RuntimeValueView, evaluate_binary, evaluate_unary,
    runtime_sequence_from_literal_values, runtime_sequence_repeat_value, runtime_value_label,
};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField, CharacterDialoguePatchOperation,
};
use std::collections::BTreeSet;
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
    format_context: crate::value::RuntimeFormatContext,
    program_owner: Option<RuntimeProgramOwner>,
    plain_text_context_template_proof:
        Option<crate::value::RuntimeDialoguePlainTextContextTemplateProof>,
}

impl VmExecutionContext {
    #[must_use]
    pub fn new(artifact: RuntimeArtifactFingerprint) -> Self {
        Self {
            artifact,
            format_context: crate::value::RuntimeFormatContext::default(),
            program_owner: None,
            plain_text_context_template_proof: None,
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
            format_context: crate::value::RuntimeFormatContext::default(),
            program_owner: Some(RuntimeProgramOwner::Awbc(program)),
            plain_text_context_template_proof: None,
        }
    }

    pub(crate) fn for_program_with_plain_text_context_proof(
        artifact: RuntimeArtifactFingerprint,
        program: std::sync::Arc<AwbcProgram>,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
    ) -> Self {
        Self {
            artifact,
            format_context: crate::value::RuntimeFormatContext::default(),
            program_owner: Some(RuntimeProgramOwner::Awbc(program)),
            plain_text_context_template_proof: Some(proof),
        }
    }

    #[must_use]
    pub const fn artifact(&self) -> RuntimeArtifactFingerprint {
        self.artifact
    }

    /// Binds the session-selected locale for new formatter attempts. A staged
    /// formatter retains its starting locale across step and save boundaries.
    #[must_use]
    pub fn with_format_context(mut self, context: crate::value::RuntimeFormatContext) -> Self {
        self.format_context = context;
        self
    }

    #[must_use]
    pub const fn format_context(&self) -> &crate::value::RuntimeFormatContext {
        &self.format_context
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

    pub(crate) fn plain_text_context_template_proof(
        &self,
        program: &AwbcProgram,
    ) -> Result<Option<crate::value::RuntimeDialoguePlainTextContextTemplateProof>, VmError> {
        let reference = program
            .validated_plain_text_context_template()
            .map_err(|error| VmError::Runtime(error.to_string()))?;
        if let Some(proof) = self.plain_text_context_template_proof
            && reference.is_none_or(|reference| !proof.matches_ref(reference))
        {
            return Err(VmError::Runtime(
                "plain-text context proof does not match the AWBC program pointer".to_owned(),
            ));
        }
        Ok(self.plain_text_context_template_proof)
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

#[derive(Debug, PartialEq)]
pub struct VmStepOutput {
    pub executed: u64,
    pub observations: Vec<VmObservation>,
    pub exit: VmExit,
}

#[derive(Debug, PartialEq)]
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
    /// A selected Need producer start awaiting the Product transaction.
    /// The instruction remains current until Product admits the producer,
    /// writes the assigned Need handle, and commits this exact cursor.
    NeedProducerStarted {
        cursor: FiberCursor,
        fiber: crate::runtime_id::RuntimePersistentFiberId,
        dst: AwbcRegisterId,
        plan: AwbcTaskPlanId,
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
        args: Vec<VmLineOperationArgument>,
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
    /// A line-root defer registration awaiting the dialogue activation owner.
    /// Captures remain in their source registers until that owner transfers
    /// affine values into the activation registry and commits this cursor.
    LineDeferRegistration {
        cursor: FiberCursor,
        site: crate::runtime_id::RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: Vec<(AwbcRegisterId, RuntimeValue)>,
    },
    /// A reached CurrentScope registration awaiting the activation owner.
    /// The owner assigns the shared registration identity and moves affine
    /// captures before advancing this exact cursor.
    ScopedDeferRegistration {
        cursor: FiberCursor,
        scope: crate::awbc::schema::AwbcScopeId,
        site: crate::runtime_id::RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: Vec<(AwbcRegisterId, RuntimeValue)>,
    },
    /// A lexical scope exit is held at its instruction until its dynamic
    /// registrations have been drained in LIFO order.
    ScopedDeferUnwind {
        cursor: FiberCursor,
        scope: crate::awbc::schema::AwbcScopeId,
    },
    /// The scope was closed after all defers ran; the activation owner must
    /// now fail the activation while preserving that completed exit filter.
    ScopedDeferFailure(crate::awbc::fiber::FiberTrap),
    /// Internal affine graph transaction evidence. This never becomes a
    /// host-facing effect; the owning executor reconciles the before/after
    /// register graph with its dialogue handle registry exactly once.
    Drop {
        policy: crate::effect::RuntimeDropPolicy,
    },
    /// Owning packet for the exact graph displaced by one assignment. The
    /// product owner journals its resource drops before publishing the write.
    DiscardedValue(RuntimeValue),
    Trap(FiberTrap),
}

/// Custody of one source register at a yielded line operation. Most operands
/// move into the observation packet; ActorLook's actor receiver remains in
/// its source register and is represented by a borrowed register coordinate.
#[derive(Debug, PartialEq)]
pub enum VmLineOperationArgument {
    BorrowedRegister(AwbcRegisterId),
    OwnedValue {
        register: AwbcRegisterId,
        value: RuntimeValue,
    },
}

#[derive(Debug, PartialEq)]
pub enum VmExit {
    Running,
    Suspended(FiberSuspensionReason),
    Returned(Option<RuntimeValue>),
    DialogueResultSelected(RuntimeValue),
    Cancelled,
    Trapped(FiberTrap),
    BudgetYield(FiberSafePoint),
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum VmError {
    #[error(transparent)]
    Evaluation(#[from] crate::value::RuntimeEvalError),
    #[error("AWBC pattern did not match its runtime value")]
    PatternMismatch,
    #[error(transparent)]
    DynamicTarget(#[from] RuntimeFlowTargetError),
    #[error("nested AWBC call exited without a value: {0:?}")]
    NestedCallExit(Box<VmNestedCallExit>),
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
    #[error("AWBC pure helper {0:?} does not exist")]
    MissingPureHelper(AwbcPureHelperId),
    #[error("AWBC trait method {0:?} does not exist")]
    MissingTraitMethod(AwbcTraitMethodId),
    #[error("AWBC line operation {0:?} does not exist")]
    MissingLineOperation(AwbcLineOperationId),
    #[error("function application expected {expected} arguments, received {actual}")]
    FunctionArgumentCount { expected: usize, actual: usize },
    #[error("runtime error: {0}")]
    Runtime(String),
}

/// Fatal non-value exit of a nested call. This remains in-memory and
/// retains budget, cancellation, and trap provenance without a string decoder.
#[derive(Clone, Debug, PartialEq)]
pub enum VmNestedCallExit {
    DialogueResultSelected(RuntimeValue),
    Cancelled,
    Trapped(FiberTrap),
    Suspended(FiberSuspensionReason),
    BudgetYield(FiberSafePoint),
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

fn instruction_call_return_point(
    site: FiberCursor,
    destination: AwbcRegisterId,
) -> Result<FiberReturnPoint, VmError> {
    let instruction_offset = site
        .instruction_offset
        .checked_add(1)
        .ok_or_else(|| VmError::Runtime("AWBC instruction call cursor overflowed".to_owned()))?;
    Ok(FiberReturnPoint {
        cursor: FiberCursor {
            instruction_offset,
            ..site
        },
        destination: Some(destination),
        continuation: FiberReturnContinuation::InstructionCall { site },
    })
}

pub trait VmHost {
    fn call_intrinsic(
        &mut self,
        program: &AwbcProgram,
        intrinsic: AwbcIntrinsicId,
        args: Vec<RuntimeValue>,
    ) -> Result<Option<RuntimeValue>, VmError>;

    /// Returns a backend result when one is available. The VM enters the
    /// verified helper body on the current fiber when this returns `None`.
    fn try_call_pure_helper(
        &mut self,
        program: &AwbcProgram,
        helper: AwbcPureHelperId,
        args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError>;

    /// Non-semantic Product telemetry for a context callback body entered on
    /// the current fiber. Called only after its frame was pushed successfully.
    fn record_context_callback_vm_call(&mut self) {}

    fn produce_character_dialogue(
        &mut self,
        _owner: &RuntimeProgramOwner,
        _operation: CharacterDialogueOperation,
        _target: RuntimeValue,
        _fields: &[CharacterDialoguePatchField<RuntimeValue>],
        _result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, VmError> {
        Err(VmError::Runtime(
            "CharacterDialogue producer is not bound to this AWBC generation".to_owned(),
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct RejectingVmHost;

impl VmHost for RejectingVmHost {
    fn call_intrinsic(
        &mut self,
        _program: &AwbcProgram,
        intrinsic: AwbcIntrinsicId,
        _args: Vec<RuntimeValue>,
    ) -> Result<Option<RuntimeValue>, VmError> {
        Err(VmError::MissingIntrinsic(intrinsic))
    }

    fn try_call_pure_helper(
        &mut self,
        _program: &AwbcProgram,
        _helper: AwbcPureHelperId,
        _args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError> {
        Ok(None)
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
                    if let VmError::Evaluation(evaluation) = &error
                        && let Some(failure) = evaluation.recoverable_expression()
                        && fiber.recover_format_operand(program, failure.to_string())?
                    {
                        executed = executed.saturating_add(1);
                        continue;
                    }
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
                if let VmError::Evaluation(evaluation) = &error
                    && let Some(failure) = evaluation.recoverable_expression()
                    && fiber.recover_format_operand(program, failure.to_string())?
                {
                    executed = executed.saturating_add(1);
                    continue;
                }
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
                let frame = fiber.active_frame()?;
                frame
                    .registers
                    .get(dst.index())
                    .ok_or(FiberStateError::RegisterOutOfBounds {
                        register: dst.0,
                        layout: frame.layout.0,
                    })?;
                let value = fiber.active_frame_mut()?.take_register(*src)?;
                fiber.active_frame_mut()?.set_register(*dst, value)?;
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
                    defers: Vec::new(),
                    defer_releasing: Vec::new(),
                    defer_exit: None,
                    defer_inflight: None,
                    defer_failure: None,
                });
        }
        AwbcInstruction::ExitScope { scope } => {
            if fiber.active_frame()?.scopes.last().map(|active| active.id) != Some(*scope) {
                return Err(VmError::Runtime(
                    "scope exit does not match the active scope".to_owned(),
                ));
            }
            let active = fiber
                .active_frame()?
                .scopes
                .last()
                .ok_or_else(|| VmError::Runtime("scope stack is empty".to_owned()))?;
            if active.defer_inflight.is_some() {
                return Err(VmError::Runtime(
                    "scope exit reached while a defer child is still active".to_owned(),
                ));
            }
            if !active.defers.is_empty() || !active.defer_releasing.is_empty() {
                observations.push(VmObservation::ScopedDeferUnwind {
                    cursor: fiber.cursor,
                    scope: *scope,
                });
                return Ok(InstructionControl::Yield);
            }
            let layout_id = fiber.active_frame()?.layout;
            let layout = program
                .frame_layouts
                .get(layout_id.index())
                .ok_or(FiberStateError::UnknownFrameLayout(layout_id.0))?;
            let frame = fiber.active_frame_mut()?;
            let defer_failure = frame.scopes.pop().and_then(|scope| {
                emit_cleanup_observations(scope.cleanups, observations);
                scope.defer_failure
            });
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
            if let Some(trap) = defer_failure {
                observations.push(VmObservation::ScopedDeferFailure(trap));
                return Ok(InstructionControl::YieldAdvanced);
            }
        }
        AwbcInstruction::BindPattern { pattern, value, .. } => {
            let value_ref = register(fiber, *value)?;
            if !test_pattern(program, *pattern, value_ref)? {
                return Err(VmError::PatternMismatch);
            }
            let value = fiber.active_frame_mut()?.take_register(*value)?;
            bind_tested_pattern_owned(program, fiber, *pattern, value)?;
        }
        AwbcInstruction::TestPattern {
            dst,
            pattern,
            value,
        } => {
            let matched = test_pattern(program, *pattern, register(fiber, *value)?)?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Bool(matched))?;
        }
        AwbcInstruction::MakeTuple { dst, items } => {
            let items = take_register_values(fiber, items)?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Tuple(items))?;
        }
        AwbcInstruction::MakeSequence { dst, items } => {
            let items = take_register_values(fiber, items)?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, runtime_sequence_from_literal_values(items))?;
        }
        AwbcInstruction::RepeatSequence { dst, value, len } => {
            let value = fiber.active_frame_mut()?.take_register(*value)?;
            let len = fiber
                .active_frame_mut()?
                .take_register(*len)?
                .try_u64()
                .and_then(|length| usize::try_from(length).ok())
                .ok_or_else(|| {
                    VmError::Runtime("repeat sequence length is not a host usize".to_owned())
                })?;
            let dst_ty = program
                .frame_layouts
                .get(fiber.active_frame()?.layout.index())
                .and_then(|layout| layout.slots.get(dst.index()))
                .map(|slot| slot.ty)
                .ok_or_else(|| {
                    VmError::Runtime("repeat sequence has no destination type".to_owned())
                })?;
            if let Some(AwbcRuntimeTypeShape::Array { length, .. }) = program
                .runtime_types
                .get(dst_ty.index())
                .map(AwbcRuntimeType::shape)
            {
                let expected = length
                    .constant()
                    .and_then(|length| usize::try_from(length).ok())
                    .ok_or_else(|| {
                        VmError::Runtime("repeat Array has no constant length".to_owned())
                    })?;
                if len != expected {
                    return Err(VmError::Runtime(format!(
                        "repeat Array length {len} differs from its destination length {expected}"
                    )));
                }
            }
            if len > 1 && !value.ownership().permits_copy() {
                return Err(VmError::Runtime(
                    "repeat sequence cannot duplicate an affine element".to_owned(),
                ));
            }
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
            let index_value = fiber.active_frame_mut()?.take_register(*index)?;
            let index =
                usize::try_from(index_value.try_u64().unwrap_or(u64::MAX)).unwrap_or(usize::MAX);
            let RuntimeValue::Seq(sequence) = fiber.active_frame_mut()?.take_register(*sequence)?
            else {
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
            let value = take_sequence_value(sequence, index)?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::SequenceSlice {
            dst,
            sequence,
            start,
        } => {
            let start = fiber.active_frame_mut()?.take_register(*start)?;
            let start = usize::try_from(start.try_u64().unwrap_or_default()).unwrap_or(usize::MAX);
            let RuntimeValue::Seq(sequence) = fiber.active_frame_mut()?.take_register(*sequence)?
            else {
                return Err(VmError::Runtime(
                    "sequence slice expected a sequence".to_owned(),
                ));
            };
            let tail = take_sequence_tail(sequence, start)?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Seq(tail))?;
        }
        AwbcInstruction::SequencePush { sequence, value } => {
            let value = fiber.active_frame_mut()?.take_register(*value)?;
            let frame = fiber.active_frame_mut()?;
            match frame
                .registers
                .get_mut(sequence.index())
                .and_then(Option::as_mut)
            {
                Some(RuntimeValue::Seq(RuntimeSeq::Values(values))) => values.push(value),
                Some(value_ref) => {
                    if !value_ref.ownership().permits_copy() {
                        return Err(VmError::Runtime(
                            "sequence push cannot duplicate an affine sequence value".to_owned(),
                        ));
                    }
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
        AwbcInstruction::SequencePopFront { dst, place } => {
            let base = match place {
                AwbcMutablePlace::Local(sequence) => *sequence,
                AwbcMutablePlace::NominalField { base, .. } => *base,
            };
            if dst == &base {
                return Err(VmError::Runtime(
                    "Vec.pop_front destination aliases its receiver".to_owned(),
                ));
            }
            let popped = {
                let frame = fiber.active_frame_mut()?;
                let Some(value) = frame
                    .registers
                    .get_mut(base.index())
                    .and_then(Option::as_mut)
                else {
                    return Err(FiberStateError::RegisterOutOfBounds {
                        register: base.0,
                        layout: frame.layout.0,
                    }
                    .into());
                };
                match (place, value) {
                    (AwbcMutablePlace::Local(_), RuntimeValue::Seq(sequence)) => {
                        sequence.pop_front()
                    }
                    (
                        AwbcMutablePlace::NominalField { field, .. },
                        RuntimeValue::NominalRecord(record),
                    ) => {
                        let field =
                            RuntimeRecordFieldId::try_from_zero_based_ordinal(*field as usize)
                                .map_err(|_| {
                                    VmError::Runtime(
                                        "Vec.pop_front has an invalid nominal field identity"
                                            .to_owned(),
                                    )
                                })?;
                        record
                            .pop_sequence_front_field(field)
                            .map_err(|error| VmError::Runtime(error.to_string()))?
                    }
                    (AwbcMutablePlace::Local(_), _) => {
                        return Err(VmError::Runtime(
                            "Vec.pop_front expected a sequence value".to_owned(),
                        ));
                    }
                    (AwbcMutablePlace::NominalField { .. }, _) => {
                        return Err(VmError::Runtime(
                            "Vec.pop_front expected a nominal record receiver".to_owned(),
                        ));
                    }
                }
            };
            let result = popped.map_or_else(RuntimeValue::option_none, RuntimeValue::option_some);
            fiber.active_frame_mut()?.set_register(*dst, result)?;
        }
        AwbcInstruction::VecPush { place, value } => {
            let value = fiber.active_frame_mut()?.take_register(*value)?;
            let frame = fiber.active_frame_mut()?;
            mutable_vec_sequence(frame, place, "Vec.push")?.push_vector_item(value);
        }
        AwbcInstruction::VecPop { dst, place } => {
            let base = mutable_place_base(place);
            if dst == &base {
                return Err(VmError::Runtime(
                    "Vec.pop destination aliases its receiver".to_owned(),
                ));
            }
            let popped = {
                let frame = fiber.active_frame_mut()?;
                mutable_vec_sequence(frame, place, "Vec.pop")?.pop_vector_item()
            };
            let result = popped.map_or_else(RuntimeValue::option_none, RuntimeValue::option_some);
            fiber.active_frame_mut()?.set_register(*dst, result)?;
        }
        AwbcInstruction::MakeRecord { dst, ty, fields } => {
            let fields = take_register_values(fiber, fields)?;
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
                .map(|payload| fiber.active_frame_mut()?.take_register(payload))
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
            let operands = take_register_values(fiber, operands)?;
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
            let state = fiber.active_frame_mut()?.take_register(*state)?;
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
            let RuntimeValue::Tuple(mut items) =
                fiber.active_frame_mut()?.take_register(*target)?
            else {
                return Err(VmError::Runtime(
                    "tuple projection expected tuple".to_owned(),
                ));
            };
            if *ordinal as usize >= items.len() {
                return Err(VmError::Runtime(
                    "tuple projection out of bounds".to_owned(),
                ));
            }
            let value = items.remove(*ordinal as usize);
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
            let target = fiber.active_frame_mut()?.take_register(*target)?;
            let value = match target {
                RuntimeValue::Record(fields) => fields
                    .into_iter()
                    .nth(field.zero_based() as usize)
                    .map(RuntimeFieldValue::into_value),
                RuntimeValue::NominalRecord(record) => record
                    .into_fields()
                    .into_iter()
                    .nth(field.zero_based() as usize),
                _ => None,
            }
            .ok_or_else(|| VmError::Runtime("record projection out of bounds".to_owned()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::ProjectField { dst, target, field } => {
            let target = fiber.active_frame_mut()?.take_register(*target)?;
            let value = match field {
                AwbcFieldProjection::Named(field) => {
                    let field = string(program, *field)?;
                    match target {
                        RuntimeValue::Record(items) => items
                            .into_iter()
                            .find(|item| item.name() == field)
                            .map(RuntimeFieldValue::into_value),
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
                    let RuntimeValue::Opaque(value) = target else {
                        return Err(VmError::Runtime(
                            "opaque-record projection expected an opaque value".to_owned(),
                        ));
                    };
                    if !owner.accepts_opaque_value(&value) {
                        return Err(VmError::Runtime(
                            "opaque-record projection rejected the target owner".to_owned(),
                        ));
                    }
                    let payload = value.into_payload();
                    let RuntimeValue::Tuple(mut fields) = payload else {
                        return Err(VmError::Runtime(
                            "opaque-record projection expected a tuple payload".to_owned(),
                        ));
                    };
                    let value = fields
                        .get_mut(*field as usize)
                        .map(|value| std::mem::replace(value, RuntimeValue::Unit))
                        .ok_or_else(|| {
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
            let value = fiber.active_frame_mut()?.take_register(*src)?;
            let value = evaluate_unary(unary_op(*op), value).map_err(VmError::Evaluation)?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::Binary { dst, op, lhs, rhs } => {
            let values = take_register_values(fiber, &[*lhs, *rhs])?;
            let [lhs, rhs] =
                values
                    .try_into()
                    .map_err(|values: Vec<_>| VmError::FunctionArgumentCount {
                        expected: 2,
                        actual: values.len(),
                    })?;
            let value = evaluate_binary(lhs, binary_op(*op), rhs).map_err(VmError::Evaluation)?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::CallPureHelper { dst, helper, args } => {
            let args = take_register_values(fiber, args)?;
            if let Some(value) = host.try_call_pure_helper(program, *helper, &args)? {
                fiber.active_frame_mut()?.set_register(*dst, value)?;
            } else {
                let function = program
                    .pure_helpers
                    .get(helper.index())
                    .ok_or(VmError::MissingPureHelper(*helper))?
                    .function;
                let return_to = instruction_call_return_point(fiber.cursor, *dst)?;
                fiber
                    .push_call_frame_with_owned_continuation(program, function, return_to, args)?;
                return Ok(InstructionControl::Transferred);
            }
        }
        AwbcInstruction::Assign { place, value } => {
            let base = match place {
                AwbcMutablePlace::Local(base) | AwbcMutablePlace::NominalField { base, .. } => {
                    *base
                }
            };
            register(fiber, base)?;
            let value = fiber.active_frame_mut()?.take_register(*value)?;
            let frame = fiber.active_frame_mut()?;
            let displaced = match place {
                AwbcMutablePlace::Local(target) => frame
                    .registers
                    .get_mut(target.index())
                    .ok_or(FiberStateError::RegisterOutOfBounds {
                        register: target.0,
                        layout: frame.layout.0,
                    })?
                    .replace(value)
                    .expect("assignment requires a live target"),
                AwbcMutablePlace::NominalField {
                    base: target,
                    field,
                } => {
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
                    replace_record_field_value(target_value, *field, value)?
                }
            };
            let handles = displaced
                .affine_line_handles()
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            if !handles.is_empty() {
                observations.push(VmObservation::DiscardedValue(displaced));
                return Ok(InstructionControl::YieldAdvanced);
            }
        }
        AwbcInstruction::CallTraitMethod {
            dst,
            method,
            receiver,
            args,
            receiver_out: _,
        } => {
            let method = program
                .trait_methods
                .get(method.index())
                .ok_or(VmError::MissingTraitMethod(*method))?;
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(fiber.active_frame_mut()?.take_register(*receiver)?);
            values.extend(take_register_values(fiber, args)?);
            let return_to = instruction_call_return_point(fiber.cursor, *dst)?;
            fiber.push_call_frame_with_owned_continuation(
                program,
                method.function,
                return_to,
                values,
            )?;
            return Ok(InstructionControl::Transferred);
        }
        AwbcInstruction::CallIntrinsic {
            dst,
            intrinsic,
            args,
        } => {
            let args = take_register_values(fiber, args)?;
            let identity = program
                .intrinsics
                .get(intrinsic.index())
                .ok_or(VmError::MissingIntrinsic(*intrinsic))?
                .identity
                .as_intrinsic();
            if let Some((kind, lazy)) = identity.and_then(context_intrinsic_kind) {
                let [receiver, message] = args.try_into().map_err(|args: Vec<RuntimeValue>| {
                    VmError::FunctionArgumentCount {
                        expected: 2,
                        actual: args.len(),
                    }
                })?;
                let dst = dst.ok_or_else(|| {
                    VmError::Runtime("context intrinsic has no result destination".to_owned())
                })?;
                match RuntimeArcError::begin_context_value(kind, receiver)
                    .map_err(context_value_error)?
                {
                    RuntimeArcErrorContextStart::Complete(value) => {
                        fiber.active_frame_mut()?.set_register(dst, value)?;
                    }
                    RuntimeArcErrorContextStart::NeedsMessage(pending) if !lazy => {
                        let value = finish_context_message(program, context, pending, message)?;
                        fiber.active_frame_mut()?.set_register(dst, value)?;
                    }
                    RuntimeArcErrorContextStart::NeedsMessage(pending) => {
                        let RuntimeValue::Callable(callable) = message else {
                            return Err(VmError::Evaluation(
                                crate::value::RuntimeEvalError::ExpectedFunction(
                                    runtime_value_label(&message),
                                ),
                            ));
                        };
                        let owner = context
                            .ok_or(VmError::MissingExecutionContext)?
                            .program_owner(program)?;
                        callable
                            .validate_for_owner(&owner)
                            .map_err(|error| VmError::Runtime(error.to_string()))?;
                        let callable_state = callable.state();
                        let arguments = callable
                            .materialize_arrow_arguments(Vec::new())
                            .map_err(|error| VmError::Runtime(error.to_string()))?;
                        let application = callable
                            .prepare_group(arguments, None)
                            .map_err(|error| VmError::Runtime(error.to_string()))?;
                        match application {
                            RuntimeCallableApplication::Complete(message) => {
                                let value =
                                    finish_context_message(program, context, pending, message)?;
                                fiber.active_frame_mut()?.set_register(dst, value)?;
                            }
                            RuntimeCallableApplication::Invoke(invocation) => {
                                enter_context_callback_frame(
                                    program,
                                    fiber,
                                    host,
                                    invocation,
                                    FiberReturnContinuation::ContextCallbackInvoke {
                                        site: fiber.cursor,
                                        pending,
                                        callable_state,
                                    },
                                )?;
                                return Ok(InstructionControl::Transferred);
                            }
                            RuntimeCallableApplication::AttachedDefault {
                                invocation,
                                pending: callable_pending,
                            } => {
                                enter_context_callback_frame(
                                    program,
                                    fiber,
                                    host,
                                    invocation,
                                    FiberReturnContinuation::ContextCallbackDefault {
                                        site: fiber.cursor,
                                        pending,
                                        callable_pending,
                                    },
                                )?;
                                return Ok(InstructionControl::Transferred);
                            }
                        }
                    }
                }
            } else if let Some(value) = host.call_intrinsic(program, *intrinsic, args)?
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
            if template.effects.len() != effects.len() {
                return Err(VmError::FunctionArgumentCount {
                    expected: template.effects.len(),
                    actual: effects.len(),
                });
            }
            let mut operand_registers = values
                .iter()
                .map(|binding| binding.value)
                .collect::<Vec<_>>();
            for (index, (binding, slot)) in effects.iter().zip(&template.effects).enumerate() {
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
                operand_registers.extend(binding.captures.iter().copied());
            }
            for (index, (binding, slot)) in values.iter().zip(&template.slots).enumerate() {
                let expected_slot =
                    crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index)
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
            }
            let mut transferred = take_register_values(fiber, &operand_registers)?.into_iter();
            let evaluated = values
                .iter()
                .zip(&template.slots)
                .map(|(_, slot)| {
                    let value = transferred.next().ok_or_else(|| {
                        VmError::Runtime("dialogue content value transfer is incomplete".to_owned())
                    })?;
                    let role = match slot.role {
                        super::schema::AwbcDialogueValueRole::Interpolation => {
                            crate::plan::RuntimeDialogueValueRole::Interpolation
                        }
                        super::schema::AwbcDialogueValueRole::Content => {
                            crate::plan::RuntimeDialogueValueRole::Content
                        }
                        super::schema::AwbcDialogueValueRole::Formatted => {
                            crate::plan::RuntimeDialogueValueRole::Formatted
                        }
                    };
                    Ok(crate::plan::RuntimeDialogueValueBinding {
                        slot: slot.slot,
                        role,
                        value,
                    })
                })
                .collect::<Result<Vec<_>, VmError>>()?;
            let owner = context.program_owner(program)?;
            let mut effect_bindings = Vec::with_capacity(effects.len());
            for (binding, slot) in effects.iter().zip(&template.effects) {
                let captures = (0..binding.captures.len())
                    .map(|_| {
                        transferred.next().ok_or_else(|| {
                            VmError::Runtime(
                                "dialogue content effect transfer is incomplete".to_owned(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let callback = dialogue_effect_callable(
                    program,
                    owner.clone(),
                    binding.state,
                    &slot.capture_types,
                    captures,
                )?;
                effect_bindings.push(crate::value::RuntimeDialogueContentEffectBinding::new(
                    binding.site,
                    callback,
                ));
            }
            if transferred.next().is_some() {
                return Err(VmError::Runtime(
                    "dialogue content received unexpected transferred values".to_owned(),
                ));
            }
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
                        super::schema::AwbcDialogueValueRole::Formatted => {
                            crate::plan::RuntimeDialogueValueRole::Formatted
                        }
                    };
                    Ok(crate::plan::RuntimeDialogueContentSlot::new(
                        slot.slot,
                        role,
                        semantic_type,
                    ))
                })
                .collect::<Result<Vec<_>, VmError>>()?;
            let value = crate::value::RuntimeDialogueContentValue::try_from_evaluated_bindings_parts_with_effect_bindings_owned(
                    context.artifact(),
                    template.id,
                    template.digest,
                    &slots,
                    evaluated,
                    effect_bindings,
                )
                .map_err(|error| VmError::Runtime(error.to_string()))?
                .into_runtime_value();
            fiber
                .active_frame_mut()?
                .set_register(*destination, value)?;
        }
        AwbcInstruction::FormatContent {
            destination,
            template,
            attempt,
            attempt_operands,
            project_method,
            project_option,
            project_result,
            operands,
        } => {
            let context = context.ok_or(VmError::MissingExecutionContext)?;
            if let Some(attempt) = attempt {
                fiber.adopt_completed_format_attempt(program, *attempt)?;
            } else {
                fiber.begin_format_content(program, context.format_context())?;
            }
            let next = fiber.format_content_state()?.next_operand();
            if let Some(operand) = operands.get(next) {
                let captures = take_register_values(fiber, &operand.captures)?;
                let site = fiber.cursor;
                fiber.push_call_frame_with_owned_continuation(
                    program,
                    operand.function,
                    FiberReturnPoint {
                        cursor: site,
                        destination: None,
                        continuation: FiberReturnContinuation::FormatOperand {
                            site,
                            ordinal: next,
                        },
                    },
                    captures,
                )?;
                return Ok(InstructionControl::Transferred);
            }
            let state = fiber.format_content_state()?;
            let state_context = state.format_context().clone();
            let mut first_recoverable = state.first_recoverable().map(str::to_owned);
            let parameters = if attempt.is_some() {
                attempt_operands
                    .iter()
                    .map(|operand| operand.parameter)
                    .collect::<Vec<_>>()
            } else {
                operands
                    .iter()
                    .map(|operand| operand.parameter)
                    .collect::<Vec<_>>()
            };
            let mut evaluated = parameters
                .into_iter()
                .zip(state.values())
                .map(|(parameter, value)| (parameter, value.clone()))
                .collect::<Vec<_>>();
            let value_index = evaluated
                .iter()
                .position(|(parameter, _)| *parameter == crate::value::RuntimeFmtParameterId::Value)
                .ok_or_else(|| {
                    VmError::Runtime("FormatContent has no verified primary operand".to_owned())
                })?;
            let original_primary = evaluated[value_index].1.clone();
            if original_primary.is_none() && first_recoverable.is_none() {
                return Err(VmError::Runtime(
                    "FormatContent primary operand has no value or recoverable error".to_owned(),
                ));
            }

            if project_method.is_none() != project_result.is_none()
                || (project_method.is_none() && *project_option)
            {
                return Err(VmError::Runtime(
                    "FormatContent project DisplayText contract is invalid".to_owned(),
                ));
            }

            let primary_kind = if let Some(method_id) = project_method {
                let result_register = project_result.expect("validated paired result temporary");
                if first_recoverable.is_none() {
                    let completed_result = fiber
                        .active_frame()?
                        .registers
                        .get(result_register.index())
                        .ok_or(FiberStateError::InvalidFrame)?
                        .clone();
                    if completed_result.is_some() {
                        let method = program
                            .trait_methods
                            .get(method_id.index())
                            .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
                        let signature = program
                            .signatures
                            .get(method.signature.index())
                            .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
                        let result_type = signature.result.ok_or_else(|| {
                            VmError::Runtime(
                                "FormatContent DisplayText method has no result type".to_owned(),
                            )
                        })?;
                        let error_type = program
                            .builtin_variant_payload_item(
                                result_type,
                                crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultErr,
                            )
                            .ok_or_else(|| {
                                VmError::Runtime(
                                    "FormatContent DisplayText method has no DisplayError type"
                                        .to_owned(),
                                )
                            })?;
                        let error_layout = program
                            .nominal_record_layout(error_type)
                            .map_err(|error| VmError::Runtime(error.to_string()))?
                            .ok_or_else(|| {
                                VmError::Runtime(
                                    "FormatContent DisplayError layout is absent".to_owned(),
                                )
                            })?;
                        let result = fiber.active_frame_mut()?.take_register(result_register)?;
                        match crate::value::project_display_result(result, &error_layout)
                            .map_err(|error| VmError::Runtime(error.to_string()))?
                        {
                            Ok(content) => {
                                evaluated[value_index].1 = Some(if *project_option {
                                    RuntimeValue::option_some(content)
                                } else {
                                    content
                                });
                            }
                            Err(reason) => first_recoverable = Some(reason),
                        }
                    } else {
                        let method = program
                            .trait_methods
                            .get(method_id.index())
                            .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
                        let signature = program
                            .signatures
                            .get(method.signature.index())
                            .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
                        let [_, context_type] = signature.params.as_slice() else {
                            return Err(VmError::Fiber(FiberStateError::InvalidFrame));
                        };
                        let original_primary = original_primary.as_ref().ok_or_else(|| {
                            VmError::Runtime(
                                "FormatContent primary operand has no value".to_owned(),
                            )
                        })?;
                        let receiver = if *project_option {
                            match original_primary
                                .clone()
                                .try_into_builtin_variant_case()
                                .map_err(|_| {
                                    VmError::Runtime(
                                        "FormatContent project Option has an invalid value"
                                            .to_owned(),
                                    )
                                })? {
                                (
                                    crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome,
                                    Some(value),
                                ) => Some(value),
                                (
                                    crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionNone,
                                    None,
                                ) => None,
                                _ => {
                                    return Err(VmError::Runtime(
                                        "FormatContent project Option has an invalid value"
                                            .to_owned(),
                                    ));
                                }
                            }
                        } else {
                            Some(original_primary.clone())
                        };
                        if let Some(receiver) = receiver {
                            let context_layout = program
                                .nominal_record_layout(*context_type)
                                .map_err(|error| VmError::Runtime(error.to_string()))?
                                .ok_or_else(|| {
                                    VmError::Runtime(
                                        "FormatContent DisplayContext layout is absent".to_owned(),
                                    )
                                })?;
                            match crate::value::project_display_context(
                                &context_layout,
                                &state_context,
                                &evaluated,
                            )
                            .map_err(|error| VmError::Runtime(error.to_string()))?
                            {
                                Err(reason) => first_recoverable = Some(reason),
                                Ok(display_context) => {
                                    let site = fiber.cursor;
                                    fiber.push_call_frame_at_owned(
                                        program,
                                        method.function,
                                        FiberReturnPoint {
                                            cursor: site,
                                            destination: Some(result_register),
                                            continuation: FiberReturnContinuation::FormatDisplay {
                                                site,
                                            },
                                        },
                                        vec![receiver, display_context],
                                    )?;
                                    return Ok(InstructionControl::Transferred);
                                }
                            }
                        }
                    }
                }
                if *project_option {
                    crate::value::RuntimeFormatPrimaryKind::OptionProjectContent
                } else {
                    crate::value::RuntimeFormatPrimaryKind::ProjectContent
                }
            } else {
                if let Some(value_operand) = attempt_operands
                    .iter()
                    .find(|operand| operand.parameter == crate::value::RuntimeFmtParameterId::Value)
                {
                    format_primary_kind_for_type(program, value_operand.ty)?
                } else {
                    let Some(value_operand) = operands.iter().find(|operand| {
                        operand.parameter == crate::value::RuntimeFmtParameterId::Value
                    }) else {
                        return Err(VmError::Runtime(
                            "FormatContent has no verified primary operand".to_owned(),
                        ));
                    };
                    format_primary_kind(program, value_operand.function)?
                }
            };
            let manifest = program
                .content_templates
                .iter()
                .find(|candidate| candidate.id == *template)
                .ok_or_else(|| VmError::Runtime("FormatContent template is absent".to_owned()))?;
            let [slot] = manifest.slots.as_slice() else {
                return Err(VmError::Runtime(
                    "FormatContent template does not have one slot".to_owned(),
                ));
            };
            let state = fiber.take_completed_format_content(program)?;
            let formatted = crate::value::finish_format_content_attempt(
                state.format_context(),
                primary_kind,
                &evaluated,
                first_recoverable.as_deref(),
            )
            .map_err(|error| VmError::Runtime(error.to_string()))?;
            let content_slot = crate::plan::RuntimeDialogueContentSlot::new(
                slot.slot,
                crate::plan::RuntimeDialogueValueRole::Formatted,
                crate::value::RuntimeDialogueOpaqueRole::Content.semantic_identity(),
            );
            let binding = crate::plan::RuntimeDialogueValueBinding {
                slot: slot.slot,
                role: crate::plan::RuntimeDialogueValueRole::Formatted,
                value: formatted.into_runtime_value(),
            };
            let value = crate::value::RuntimeDialogueContentValue::try_from_evaluated_bindings_parts_with_effect_bindings_owned(
                context.artifact(),
                manifest.id,
                manifest.digest,
                &[content_slot],
                vec![binding],
                Vec::new(),
            )
            .map_err(|error| VmError::Runtime(error.to_string()))?
            .into_runtime_value();
            fiber
                .active_frame_mut()?
                .set_register(*destination, value)?;
        }
        AwbcInstruction::FormatOperandAttempt { attempt, parameter } => {
            let context = context.ok_or(VmError::MissingExecutionContext)?;
            fiber.begin_format_operand_attempt(
                program,
                context.format_context(),
                *attempt,
                *parameter,
            )?;
        }
        AwbcInstruction::CompleteFormatOperand {
            attempt,
            parameter,
            value,
        } => {
            fiber.complete_format_operand_attempt(program, *attempt, *parameter, *value)?;
        }
        AwbcInstruction::AbandonFormatAttempt { attempt } => {
            fiber.abandon_format_operand_attempt(*attempt)?;
        }
        AwbcInstruction::CharacterDialogue {
            destination,
            operation,
            target,
            fields,
        } => {
            let context = context.ok_or(VmError::MissingExecutionContext)?;
            let owner = context.program_owner(program)?;
            let mut operand_registers = vec![*target];
            operand_registers.extend(fields.iter().filter_map(|field| match &field.operation {
                CharacterDialoguePatchOperation::Set(source) => Some(*source),
                CharacterDialoguePatchOperation::Clear => None,
            }));
            let mut transferred = take_register_values(fiber, &operand_registers)?.into_iter();
            let target = transferred.next().ok_or_else(|| {
                VmError::Runtime("CharacterDialogue target transfer is missing".to_owned())
            })?;
            let mut evaluated = Vec::with_capacity(fields.len());
            for field in fields {
                let operation = match &field.operation {
                    CharacterDialoguePatchOperation::Set(_) => {
                        CharacterDialoguePatchOperation::Set(transferred.next().ok_or_else(
                            || {
                                VmError::Runtime(
                                    "CharacterDialogue field transfer is incomplete".to_owned(),
                                )
                            },
                        )?)
                    }
                    CharacterDialoguePatchOperation::Clear => {
                        CharacterDialoguePatchOperation::Clear
                    }
                };
                evaluated.push(CharacterDialoguePatchField {
                    coordinate: field.coordinate.clone(),
                    operation,
                });
            }
            if transferred.next().is_some() {
                return Err(VmError::Runtime(
                    "CharacterDialogue received unexpected transferred values".to_owned(),
                ));
            }
            let frame = fiber.active_frame()?;
            let layout =
                program
                    .frame_layouts
                    .get(frame.layout.index())
                    .ok_or(VmError::Runtime(
                        "CharacterDialogue frame layout is missing".to_owned(),
                    ))?;
            let result_type = layout
                .slots
                .get(destination.index())
                .ok_or(VmError::Runtime(
                    "CharacterDialogue destination is missing".to_owned(),
                ))?
                .ty;
            let semantic_type = program
                .runtime_types
                .get(result_type.index())
                .ok_or(VmError::MissingType(result_type))?
                .semantic_identity();
            let value = host.produce_character_dialogue(
                &owner,
                *operation,
                target,
                &evaluated,
                semantic_type,
            )?;
            if !runtime_value_matches_type(program, &value, result_type, 0) {
                return Err(VmError::Runtime(
                    "CharacterDialogue producer returned an invalid value".to_owned(),
                ));
            }
            fiber
                .active_frame_mut()?
                .set_register(*destination, value)?;
        }
        AwbcInstruction::EmitEffect { effect, args } => {
            let args = take_register_values(fiber, args)?;
            observations.push(VmObservation::Effect {
                effect: *effect,
                args,
            });
        }
        AwbcInstruction::RegisterCleanup { key, effect, args } => {
            let key = string(program, *key)?.to_owned();
            let args = take_register_values(fiber, args)?;
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
        AwbcInstruction::RegisterDefer {
            site,
            outcome,
            owner,
            captures,
        } => {
            let captured = captures
                .iter()
                .copied()
                .zip(take_register_values(fiber, captures)?)
                .collect::<Vec<_>>();
            if *owner == super::schema::AwbcDeferOwner::LineRoot {
                observations.push(VmObservation::LineDeferRegistration {
                    cursor: fiber.cursor,
                    site: *site,
                    outcome: *outcome,
                    captures: captured,
                });
                return Ok(InstructionControl::Yield);
            }
            let scope = fiber
                .active_frame()?
                .scopes
                .last()
                .map(|scope| scope.id)
                .ok_or_else(|| {
                    VmError::Runtime(
                        "CurrentScope defer requires an active lexical scope".to_owned(),
                    )
                })?;
            observations.push(VmObservation::ScopedDeferRegistration {
                cursor: fiber.cursor,
                scope,
                site: *site,
                outcome: *outcome,
                captures: captured,
            });
            return Ok(InstructionControl::Yield);
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
            let captures = take_register_values(fiber, captures)?;
            let callable = RuntimeCallableValue::try_new(owner, *state, captures)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Callable(callable))?;
        }
        AwbcInstruction::SpecializeCallable {
            dst,
            src,
            specialization,
        } => {
            let owner = context
                .ok_or(VmError::MissingExecutionContext)?
                .program_owner(program)?;
            let source = register(fiber, *src)?.clone();
            let RuntimeValue::Callable(callable) = source else {
                return Err(VmError::Runtime(format!(
                    "callable specialization expected callable, found {}",
                    runtime_value_label(&source)
                )));
            };
            let callable = callable
                .specialize(&owner, *specialization)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Callable(callable))?;
        }
        AwbcInstruction::ApplyGroup { dst, callee, args } => {
            let RuntimeValue::Callable(callable) = register(fiber, *callee)? else {
                let callee_value = register(fiber, *callee)?;
                return Err(VmError::Runtime(format!(
                    "callable application expected callable, found {}",
                    runtime_value_label(callee_value)
                )));
            };
            let argument_refs = args
                .iter()
                .map(|argument| register(fiber, *argument))
                .collect::<Result<Vec<_>, _>>()?;
            callable
                .inspect_arrow_arguments(&argument_refs)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            let mut operands = Vec::with_capacity(args.len() + 1);
            operands.push(*callee);
            operands.extend(args.iter().copied());
            let mut values = take_register_values(fiber, &operands)?.into_iter();
            let RuntimeValue::Callable(callable) = values.next().ok_or_else(|| {
                VmError::Runtime("callable application lost its callee operand".to_owned())
            })?
            else {
                return Err(VmError::Runtime(
                    "callable application callee changed after preflight".to_owned(),
                ));
            };
            let args = values.collect();
            return apply_runtime_callable(program, fiber, context, callable, args, *dst);
        }
        AwbcInstruction::StartNeed { dst, plan, args } => {
            let args = take_register_values(fiber, args)?;
            observations.push(VmObservation::NeedProducerStarted {
                cursor: fiber.cursor,
                fiber: crate::runtime_id::RuntimePersistentFiberId::from_allocated(
                    fiber.instance.get().get(),
                ),
                dst: *dst,
                plan: *plan,
                args,
            });
            return Ok(InstructionControl::Yield);
        }
        AwbcInstruction::SpawnFiber {
            dst,
            function,
            args,
        } => {
            let args = take_register_values(fiber, args)?;
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
            let observed_value = {
                let proof = RuntimeStreamYieldCopyProof::inspect(register(fiber, *value)?)
                    .map_err(|error| VmError::Runtime(error.to_string()))?;
                proof.copy()
            };
            let value = fiber.active_frame_mut()?.take_register(*value)?;
            observations.push(VmObservation::StreamYield {
                stream: *stream,
                value: observed_value,
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
            let operation_row = program
                .line_operations
                .get(operation.index())
                .ok_or(VmError::MissingLineOperation(*operation))?;
            let mut seen = BTreeSet::new();
            if args.iter().any(|register| !seen.insert(*register)) {
                return Err(VmError::Runtime(
                    "line-operation operands use one register more than once".to_owned(),
                ));
            }
            let observed_args = match operation_row {
                AwbcLineOperation::ActorLook { .. } => {
                    if args.len() != 3 {
                        return Err(VmError::Runtime(
                            "ActorLook requires a borrowed actor and two owned operands".to_owned(),
                        ));
                    }
                    register(fiber, args[0])?;
                    let mut values = take_register_values(fiber, &args[1..])?.into_iter();
                    let look = values
                        .next()
                        .expect("ActorLook's second operand was preflighted");
                    let crossfade = values
                        .next()
                        .expect("ActorLook's third operand was preflighted");
                    vec![
                        VmLineOperationArgument::BorrowedRegister(args[0]),
                        VmLineOperationArgument::OwnedValue {
                            register: args[1],
                            value: look,
                        },
                        VmLineOperationArgument::OwnedValue {
                            register: args[2],
                            value: crossfade,
                        },
                    ]
                }
                AwbcLineOperation::AcquireActor { .. }
                | AwbcLineOperation::Schedule { .. }
                | AwbcLineOperation::VoiceHandle { .. } => take_register_values(fiber, args)?
                    .into_iter()
                    .zip(args.iter().copied())
                    .map(|(value, register)| VmLineOperationArgument::OwnedValue {
                        register,
                        value,
                    })
                    .collect(),
            };
            observations.push(VmObservation::LineOperation {
                cursor: fiber.cursor,
                dst: *dst,
                operation: *operation,
                args: observed_args,
            });
            return Ok(InstructionControl::Yield);
        }
        AwbcInstruction::CommitDialogueResult { source } => {
            observations.push(VmObservation::DialogueResult {
                cursor: fiber.cursor,
                source_register: *source,
                source: fiber.active_frame_mut()?.take_register(*source)?,
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
    callable: RuntimeCallableValue,
    args: Vec<RuntimeValue>,
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
    let logical_arguments = callable
        .materialize_arrow_arguments(args)
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    let application = callable
        .prepare_group(logical_arguments, None)
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
            fiber.push_call_frame_at_owned(program, function, return_to, values)?;
            Ok(InstructionControl::Transferred)
        }
        RuntimeCallableApplication::AttachedDefault {
            invocation,
            pending,
        } => {
            let function = invocation_function(&invocation)?;
            let values = invocation_values(invocation)?;
            let return_to = FiberReturnPoint {
                cursor: return_cursor,
                destination: None,
                continuation: FiberReturnContinuation::ApplyGroupDefault {
                    pending,
                    destination,
                },
            };
            fiber.push_call_frame_with_owned_continuation(program, function, return_to, values)?;
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

fn context_intrinsic_kind(
    intrinsic: crate::value::RuntimeIntrinsic,
) -> Option<(RuntimeArcErrorContextKind, bool)> {
    use crate::value::RuntimeIntrinsic;
    match intrinsic {
        RuntimeIntrinsic::StdResultContext => Some((RuntimeArcErrorContextKind::Result, false)),
        RuntimeIntrinsic::StdResultWithContext => Some((RuntimeArcErrorContextKind::Result, true)),
        RuntimeIntrinsic::StdOptionContext => Some((RuntimeArcErrorContextKind::Option, false)),
        RuntimeIntrinsic::StdOptionWithContext => Some((RuntimeArcErrorContextKind::Option, true)),
        _ => None,
    }
}

fn context_value_error(error: crate::value::RuntimeArcErrorValueError) -> VmError {
    VmError::Evaluation(crate::value::RuntimeEvalError::DialogueContentConstruction(
        error.to_string(),
    ))
}

fn finish_context_message(
    program: &AwbcProgram,
    context: Option<&VmExecutionContext>,
    pending: RuntimeArcErrorContextPending,
    message: RuntimeValue,
) -> Result<RuntimeValue, VmError> {
    let context = context.ok_or(VmError::MissingExecutionContext)?;
    let limits = crate::entry::RuntimeSchemaLimits::engine_default();
    let proof = context.plain_text_context_template_proof(program)?;
    let message_content = RuntimeDialogueContentValue::try_new_context_message_with_limits(
        context.artifact(),
        proof,
        message,
        limits,
    )
    .map_err(|error| {
        VmError::Evaluation(crate::value::RuntimeEvalError::DialogueContentConstruction(
            error.to_string(),
        ))
    })?;
    RuntimeArcError::finish_context_value(
        pending,
        message_content,
        RuntimeArcErrorFrame::empty(),
        limits,
    )
    .map_err(context_value_error)
}

fn enter_context_callback_frame(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    host: &mut impl VmHost,
    invocation: RuntimeCallableInvocation,
    continuation: FiberReturnContinuation,
) -> Result<(), VmError> {
    let function = invocation_function(&invocation)?;
    let values = invocation_values(invocation)?;
    fiber.push_call_frame_with_owned_continuation(
        program,
        function,
        FiberReturnPoint {
            cursor: fiber.cursor,
            destination: None,
            continuation,
        },
        values,
    )?;
    host.record_context_callback_vm_call();
    Ok(())
}

fn context_call_at_site(
    program: &AwbcProgram,
    site: FiberCursor,
) -> Result<(AwbcRegisterId, AwbcRegisterId), VmError> {
    let block = program
        .blocks
        .get(site.block.index())
        .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
    let instruction = block
        .instructions
        .start
        .checked_add(site.instruction_offset)
        .and_then(|index| program.instructions.get(index as usize))
        .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
    let AwbcInstruction::CallIntrinsic {
        dst: Some(dst),
        intrinsic,
        args,
    } = instruction
    else {
        return Err(VmError::Fiber(FiberStateError::InvalidFrame));
    };
    if block.owner != site.function
        || site.instruction_offset >= block.instructions.len
        || !matches!(
            program
                .intrinsics
                .get(intrinsic.index())
                .and_then(|record| record.identity.as_intrinsic()),
            Some(
                crate::value::RuntimeIntrinsic::StdResultWithContext
                    | crate::value::RuntimeIntrinsic::StdOptionWithContext
            )
        )
    {
        return Err(VmError::Fiber(FiberStateError::InvalidFrame));
    }
    let [_, callback] = args.as_slice() else {
        return Err(VmError::Fiber(FiberStateError::InvalidFrame));
    };
    Ok((*dst, *callback))
}

fn complete_context_callback_return(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    host: &mut impl VmHost,
    context: Option<&VmExecutionContext>,
    continuation: FiberReturnContinuation,
    value: Option<RuntimeValue>,
) -> Result<(), VmError> {
    let (site, pending, message) = match continuation {
        FiberReturnContinuation::ContextCallbackDefault {
            site,
            pending,
            callable_pending,
        } => {
            let default_value = value.ok_or_else(|| {
                VmError::Runtime("context callback default returned no value".to_owned())
            })?;
            let callable_state = callable_pending.state();
            let application = callable_pending
                .complete_default(default_value)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            match application {
                RuntimeCallableApplication::Complete(message) => (site, pending, message),
                RuntimeCallableApplication::Invoke(invocation) => {
                    enter_context_callback_frame(
                        program,
                        fiber,
                        host,
                        invocation,
                        FiberReturnContinuation::ContextCallbackInvoke {
                            site,
                            pending,
                            callable_state,
                        },
                    )?;
                    return Ok(());
                }
                RuntimeCallableApplication::AttachedDefault { .. } => {
                    return Err(VmError::Runtime(
                        "context callback selected another default stage".to_owned(),
                    ));
                }
            }
        }
        FiberReturnContinuation::ContextCallbackInvoke { site, pending, .. } => {
            let message = value.ok_or_else(|| {
                VmError::Runtime("context callback returned no message".to_owned())
            })?;
            (site, pending, message)
        }
        _ => return Err(VmError::Fiber(FiberStateError::InvalidFrame)),
    };
    if fiber.cursor != site {
        return Err(VmError::Fiber(FiberStateError::InvalidFrame));
    }
    let (dst, _) = context_call_at_site(program, site)?;
    let next_offset = site
        .instruction_offset
        .checked_add(1)
        .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
    let result = finish_context_message(program, context, pending, message)?;
    let destination_type = program
        .frame_layouts
        .get(fiber.active_frame()?.layout.index())
        .and_then(|layout| layout.slots.get(dst.index()))
        .map(|slot| slot.ty)
        .ok_or(VmError::Fiber(FiberStateError::InvalidFrame))?;
    require_runtime_type(program, destination_type, &result)?;
    fiber.active_frame_mut()?.set_register(dst, result)?;
    fiber.cursor.instruction_offset = next_offset;
    Ok(())
}

fn replace_record_field_value(
    target: &mut RuntimeValue,
    field: u32,
    value: RuntimeValue,
) -> Result<RuntimeValue, VmError> {
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
    host: &mut impl VmHost,
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
        AwbcTerminator::SequenceNext {
            sequence,
            item,
            some_block,
            none_block,
        } => {
            if sequence == item {
                return Err(VmError::Runtime(
                    "sequence next item aliases its owned source".to_owned(),
                ));
            }
            let frame = fiber.active_frame_mut()?;
            if frame
                .registers
                .get(item.index())
                .is_none_or(Option::is_some)
            {
                return Err(VmError::Runtime(
                    "sequence next requires a vacant item register".to_owned(),
                ));
            }
            let popped = match frame.registers.get_mut(sequence.index()) {
                Some(Some(RuntimeValue::Seq(values))) => values.pop_front(),
                _ => {
                    return Err(VmError::Runtime(
                        "sequence next requires an owned sequence".to_owned(),
                    ));
                }
            };
            if let Some(value) = popped {
                fiber.active_frame_mut()?.set_register(*item, value)?;
                jump(fiber, *some_block);
            } else {
                jump(fiber, *none_block);
            }
            Ok(VmExit::Running)
        }
        AwbcTerminator::Match {
            scrutinee,
            arms,
            default,
        } => {
            let value = fiber.active_frame_mut()?.take_register(*scrutinee)?;
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
            let args = take_register_values(fiber, args)?;
            fiber.push_call_frame_with_owned_args(program, *function, *resume, *dst, args)?;
            Ok(VmExit::Running)
        }
        AwbcTerminator::ProjectCall { call } => execute_project_call(program, fiber, context, call),
        AwbcTerminator::GotoStatic { function, args } => {
            let args = take_register_values(fiber, args)?;
            emit_unwind_cleanup_observations(fiber, observations);
            fiber.replace_root_function_owned(program, *function, args)?;
            observations.push(VmObservation::Goto(*function));
            Ok(VmExit::Running)
        }
        AwbcTerminator::GotoDynamic { target, args } => {
            let target_value = fiber.active_frame_mut()?.take_register(*target)?;
            let target = match &target_value {
                RuntimeValue::String(target) => program
                    .resolve_flow_target_value(target)
                    .map(|(_, function)| function)
                    .map_err(VmError::DynamicTarget)?,
                RuntimeValue::EntityRef(target) => program
                    .resolve_flow_target_value(&target.runtime_label())
                    .map(|(_, function)| function)
                    .map_err(VmError::DynamicTarget)?,
                _ => {
                    return Err(VmError::Runtime(format!(
                        "invalid dynamic goto target `{}`",
                        runtime_value_label(&target_value)
                    )));
                }
            };
            let args = take_register_values(fiber, args)?;
            emit_unwind_cleanup_observations(fiber, observations);
            fiber.replace_root_function_owned(program, target, args)?;
            observations.push(VmObservation::Goto(target));
            Ok(VmExit::Running)
        }
        AwbcTerminator::Dialogue {
            target,
            content,
            values,
            effects,
            line_task_captures,
            result,
            resume,
        } => {
            let context = context.ok_or(VmError::MissingExecutionContext)?;
            let _owner = context.program_owner(program)?;
            let frame = fiber.active_frame()?;
            let layout = program
                .frame_layouts
                .get(frame.layout.index())
                .ok_or_else(|| VmError::Runtime("dialogue frame layout is missing".to_owned()))?;
            let target_type = layout
                .slots
                .get(target.index())
                .ok_or_else(|| VmError::Runtime("dialogue target register is missing".to_owned()))?
                .ty;
            let mut operand_registers = Vec::with_capacity(
                1 + values.len()
                    + effects
                        .iter()
                        .map(|effect| effect.captures.len())
                        .sum::<usize>()
                    + line_task_captures.len(),
            );
            operand_registers.push(*target);
            operand_registers.extend(values.iter().map(|binding| binding.value));
            for effect in effects {
                operand_registers.extend(effect.captures.iter().copied());
            }
            operand_registers.extend(line_task_captures.iter().copied());
            let mut transferred = take_register_values(fiber, &operand_registers)?.into_iter();
            let target_value = transferred.next().ok_or_else(|| {
                VmError::Runtime("dialogue target transfer is missing".to_owned())
            })?;
            if !super::fiber::dialogue_target_matches_program(program, target_type, &target_value) {
                return Err(VmError::Runtime(
                    "dialogue target is not admitted by this AWBC executable".to_owned(),
                ));
            }
            let RuntimeValue::Opaque(target_value) = target_value else {
                return Err(VmError::Runtime(
                    "dialogue target register is not an opaque value".to_owned(),
                ));
            };
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
                            crate::awbc::schema::AwbcDialogueValueRole::Formatted => {
                                crate::plan::RuntimeDialogueValueRole::Formatted
                            }
                        },
                        value: transferred.next().ok_or_else(|| {
                            VmError::Runtime("dialogue value transfer is incomplete".to_owned())
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, VmError>>()?
                .into_boxed_slice();
            let effects = effects
                .iter()
                .map(|binding| {
                    let captures = (0..binding.captures.len())
                        .map(|_| {
                            transferred.next().ok_or_else(|| {
                                VmError::Runtime(
                                    "dialogue effect capture transfer is incomplete".to_owned(),
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(super::fiber::FiberDialogueContentEffectBinding {
                        site: binding.site,
                        state: binding.state,
                        captures: captures.into_boxed_slice(),
                    })
                })
                .collect::<Result<Vec<_>, VmError>>()?
                .into_boxed_slice();
            let line_task_captures = line_task_captures
                .iter()
                .map(|_| {
                    transferred.next().ok_or_else(|| {
                        VmError::Runtime("dialogue line capture transfer is incomplete".to_owned())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            if transferred.next().is_some() {
                return Err(VmError::Runtime(
                    "dialogue terminator received unexpected transferred values".to_owned(),
                ));
            }
            suspend(
                fiber,
                *resume,
                FiberSuspensionReason::Dialogue {
                    target: Some(target_value),
                    target_type,
                    content: *content,
                    values,
                    effects,
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
            let plan_record = program
                .task_plans
                .get(plan.index())
                .ok_or_else(|| VmError::Runtime("AwaitMany plan is absent".to_owned()))?;
            let signature = program
                .signatures
                .get(plan_record.signature.index())
                .ok_or_else(|| VmError::Runtime("AwaitMany signature is absent".to_owned()))?;
            let source_value = fiber.active_frame()?.register(*source)?;
            if signature.params.len() == 1 && !source_value.ownership().permits_copy() {
                return Err(VmError::Runtime(
                    "AwaitMany host payload items require deep Copy values".to_owned(),
                ));
            }
            let source = fiber.active_frame_mut()?.take_register(*source)?;
            let items = match source {
                RuntimeValue::Seq(sequence) => sequence.into_values(),
                value => vec![value],
            };
            let results = vec![None; items.len()];
            suspend(
                fiber,
                *resume,
                FiberSuspensionReason::AwaitMany(FiberAwaitManyState {
                    plan: *plan,
                    binding: *binding,
                    invocation: None,
                    items,
                    next_index: 0,
                    in_flight: Vec::new(),
                    results,
                }),
            )
        }
        AwbcTerminator::HostCall {
            call,
            args,
            dst,
            resume,
        } => {
            let argument_values = args
                .iter()
                .map(|register| fiber.active_frame()?.register(*register))
                .collect::<Result<Vec<_>, _>>()?;
            validate_host_call_values(program, *call, &argument_values)?;
            let args = take_register_values(fiber, args)?;
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
                .map(|value| fiber.active_frame_mut()?.take_register(value))
                .transpose()?;
            drain_active_frame_cleanups(fiber, observations)?;
            let returning_function = fiber.active_frame()?.function;
            let (return_to, returned_value) =
                fiber.finish_return_with_continuation(program, value)?;
            if let Some(return_to) = return_to {
                let FiberReturnPoint {
                    cursor,
                    destination,
                    continuation,
                } = return_to;
                match continuation {
                    FiberReturnContinuation::Ordinary
                    | FiberReturnContinuation::FormatOperand { .. }
                    | FiberReturnContinuation::FormatDisplay { .. }
                    | FiberReturnContinuation::InstructionCall { .. } => {}
                    continuation @ (FiberReturnContinuation::ContextCallbackDefault { .. }
                    | FiberReturnContinuation::ContextCallbackInvoke { .. }) => {
                        complete_context_callback_return(
                            program,
                            fiber,
                            host,
                            context,
                            continuation,
                            returned_value,
                        )?;
                    }
                    continuation => {
                        complete_project_call_return(
                            program,
                            fiber,
                            returning_function,
                            FiberReturnPoint {
                                cursor,
                                destination,
                                continuation,
                            },
                            returned_value,
                        )?;
                    }
                }
                Ok(VmExit::Running)
            } else {
                Ok(terminal_exit(fiber))
            }
        }
        AwbcTerminator::SelectDialogueResult { value } => {
            if fiber.frames.len() != 1 {
                return Err(VmError::Runtime(
                    "dialogue result selection cannot terminate a nested call frame".to_owned(),
                ));
            }
            let value = fiber.active_frame_mut()?.take_register(*value)?;
            drain_active_frame_cleanups(fiber, observations)?;
            fiber.mark_dialogue_result_selected(value)?;
            Ok(terminal_exit(fiber))
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
    let RuntimeValue::Callable(callable) = register(fiber, call.callee)? else {
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

    let mut copied_spreads = vec![None::<Vec<RuntimeValue>>; call.operands.len()];
    for (index, operand) in call.operands.iter().enumerate() {
        if operand.mode != AwbcProjectCallOperandMode::Spread {
            continue;
        }
        let value = register(fiber, operand.value)?;
        match value {
            RuntimeValue::Tuple(_) => {}
            RuntimeValue::Seq(sequence) if sequence.as_values().is_some() => {}
            RuntimeValue::Seq(sequence) if sequence.ownership().permits_copy() => {
                copied_spreads[index] = Some(
                    (0..sequence.len())
                        .map(|ordinal| sequence.value_at(ordinal))
                        .collect(),
                );
            }
            RuntimeValue::Seq(_) => {
                return Err(VmError::Runtime(
                    "an affine project-call spread must use transferable value storage".to_owned(),
                ));
            }
            _ => {
                return Err(VmError::Runtime(format!(
                    "project-call spread operand is not a sequence: {}",
                    runtime_value_label(value)
                )));
            }
        }
    }
    let operand_refs = call
        .operands
        .iter()
        .enumerate()
        .map(|(index, operand)| {
            let value = register(fiber, operand.value)?;
            Ok(match (operand.mode, value) {
                (AwbcProjectCallOperandMode::Value, value) => vec![value],
                (AwbcProjectCallOperandMode::Spread, RuntimeValue::Tuple(values)) => {
                    values.iter().collect()
                }
                (AwbcProjectCallOperandMode::Spread, RuntimeValue::Seq(sequence)) => {
                    if let Some(values) = sequence.as_values() {
                        values.iter().collect()
                    } else {
                        copied_spreads[index]
                            .as_ref()
                            .expect("copyable spread was previewed")
                            .iter()
                            .collect()
                    }
                }
                (AwbcProjectCallOperandMode::Spread, _) => {
                    unreachable!("spread shape was checked before borrowed source projection")
                }
            })
        })
        .collect::<Result<Vec<Vec<&RuntimeValue>>, VmError>>()?;

    enum ProjectCallMaterializedRow<'a> {
        Fixed(&'a RuntimeValue),
        Rest(Vec<&'a RuntimeValue>),
    }

    let source_count = operand_refs.iter().map(Vec::len).sum::<usize>();
    let mut used_sources = BTreeSet::new();
    let mut materialized_rows = Vec::with_capacity(call.ordinary.len());
    if call.ordinary.len() != state.parameters.len() {
        return Err(VmError::Runtime(
            "project-call logical parameters do not match the selected callable state".to_owned(),
        ));
    }
    for (parameter_index, row) in call.ordinary.iter().enumerate() {
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
                insert_project_call_source(
                    &mut used_sources,
                    *source_index,
                    source_count,
                    "project-call source",
                )?;
                materialized_rows.push(ProjectCallMaterializedRow::Fixed(project_call_source_ref(
                    &operand_refs,
                    *source_index,
                )?));
            }
            AwbcProjectCallOrdinaryMaterialization::Rest { source_indices, .. } => {
                let mut values = Vec::new();
                for source_index in source_indices {
                    insert_project_call_source(
                        &mut used_sources,
                        *source_index,
                        source_count,
                        "project-call source",
                    )?;
                    values.push(project_call_source_ref(&operand_refs, *source_index)?);
                }
                materialized_rows.push(ProjectCallMaterializedRow::Rest(values));
            }
        }
    }
    let attached_ref = match &call.attached {
        None => None,
        Some(attached) => match &attached.presence {
            AwbcProjectCallAttachedPresence::RequiredPresent
            | AwbcProjectCallAttachedPresence::OptionalPresent
            | AwbcProjectCallAttachedPresence::DefaultedPresent => {
                let source_index = attached.source_index.ok_or_else(|| {
                    VmError::Runtime("present attached project-call source is absent".to_owned())
                })?;
                insert_project_call_source(
                    &mut used_sources,
                    source_index,
                    source_count,
                    "attached project-call source",
                )?;
                Some(project_call_source_ref(&operand_refs, source_index)?)
            }
            AwbcProjectCallAttachedPresence::OptionalOmitted
            | AwbcProjectCallAttachedPresence::DefaultedOmitted => None,
        },
    };
    if used_sources.len() != source_count {
        return Err(VmError::Runtime(
            "project-call left a physical operand source unused".to_owned(),
        ));
    }
    let inspected_rows = materialized_rows
        .iter()
        .map(|row| match row {
            ProjectCallMaterializedRow::Fixed(value) => {
                RuntimeCallableMaterializedArgument::Fixed(value)
            }
            ProjectCallMaterializedRow::Rest(values) => {
                RuntimeCallableMaterializedArgument::Rest(values)
            }
        })
        .collect::<Vec<_>>();
    callable
        .inspect_group_materialization(&inspected_rows, attached_ref)
        .map_err(|error| VmError::Runtime(error.to_string()))?;

    let mut transfer_registers = Vec::with_capacity(call.operands.len() + 1);
    transfer_registers.push(call.callee);
    transfer_registers.extend(call.operands.iter().map(|operand| operand.value));
    let mut transferred = take_register_values(fiber, &transfer_registers)?.into_iter();
    let RuntimeValue::Callable(callable) = transferred
        .next()
        .ok_or_else(|| VmError::Runtime("project-call lost its callee".to_owned()))?
    else {
        return Err(VmError::Runtime(
            "project-call callee changed after borrowed preflight".to_owned(),
        ));
    };
    let physical = transferred.collect::<Vec<_>>();
    let mut operands = Vec::with_capacity(call.operands.len());
    for (operand, value) in call.operands.iter().zip(physical) {
        operands.push(match operand.mode {
            AwbcProjectCallOperandMode::Value => vec![Some(value)],
            AwbcProjectCallOperandMode::Spread => {
                crate::value::runtime_value_into_sequence_values(value)
                    .map_err(|value| {
                        VmError::Runtime(format!(
                            "project-call spread operand changed after preflight: {}",
                            runtime_value_label(&value)
                        ))
                    })?
                    .into_iter()
                    .map(Some)
                    .collect()
            }
        });
    }
    let mut logical_values = Vec::with_capacity(call.ordinary.len());
    for row in &call.ordinary {
        logical_values.push(match row {
            AwbcProjectCallOrdinaryMaterialization::Fixed { source_index, .. } => {
                take_project_call_source_value(&mut operands, *source_index)?
            }
            AwbcProjectCallOrdinaryMaterialization::Rest { source_indices, .. } => {
                let mut values = Vec::new();
                for source_index in source_indices {
                    values.push(take_project_call_source_value(
                        &mut operands,
                        *source_index,
                    )?);
                }
                crate::value::runtime_sequence_values(values)
            }
        });
    }
    let attached = match &call.attached {
        Some(attached)
            if matches!(
                attached.presence,
                AwbcProjectCallAttachedPresence::RequiredPresent
                    | AwbcProjectCallAttachedPresence::OptionalPresent
                    | AwbcProjectCallAttachedPresence::DefaultedPresent
            ) =>
        {
            Some(take_project_call_source_value(
                &mut operands,
                attached.source_index.ok_or_else(|| {
                    VmError::Runtime("present attached project-call source is absent".to_owned())
                })?,
            )?)
        }
        _ => None,
    };
    let application = callable
        .prepare_group(logical_values, attached)
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
            bind_pattern_owned(program, fiber, call.result_pattern, value)?;
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
            fiber.push_call_frame_with_owned_continuation(program, function, return_to, args)?;
        }
        RuntimeCallableApplication::AttachedDefault {
            invocation,
            pending,
        } => {
            let function = invocation_function(&invocation)?;
            let args = invocation_values(invocation)?;
            let point = project_call_return_point(program, fiber, call.resume, site)?;
            let return_to = FiberReturnPoint {
                continuation: FiberReturnContinuation::ProjectCallDefault { site, pending },
                ..point
            };
            fiber.push_call_frame_with_owned_continuation(program, function, return_to, args)?;
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
    value: Option<RuntimeValue>,
) -> Result<(), VmError> {
    let FiberReturnPoint {
        cursor: return_cursor,
        destination: _,
        continuation,
    } = return_to;
    let continuation = match continuation {
        FiberReturnContinuation::ApplyGroupDefault {
            pending,
            destination,
        } => {
            let default_value = value.ok_or_else(|| {
                VmError::Runtime("callable default returned no attached value".to_owned())
            })?;
            let application = pending
                .complete_default(default_value)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            match application {
                RuntimeCallableApplication::Complete(value) => {
                    fiber.active_frame_mut()?.set_register(destination, value)?;
                }
                RuntimeCallableApplication::Invoke(invocation) => {
                    let function = invocation_function(&invocation)?;
                    let args = invocation_values(invocation)?;
                    let return_to = FiberReturnPoint::ordinary(return_cursor, Some(destination));
                    fiber.push_call_frame_with_owned_continuation(
                        program, function, return_to, args,
                    )?;
                }
                RuntimeCallableApplication::AttachedDefault { .. } => {
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
        FiberReturnContinuation::Ordinary
        | FiberReturnContinuation::FormatOperand { .. }
        | FiberReturnContinuation::FormatDisplay { .. }
        | FiberReturnContinuation::InstructionCall { .. }
        | FiberReturnContinuation::ContextCallbackDefault { .. }
        | FiberReturnContinuation::ContextCallbackInvoke { .. } => {
            return Ok(());
        }
    };
    let call = project_call_at_site(program, site)?;
    let resume = program
        .resume_points
        .get(call.resume.index())
        .ok_or(VmError::Fiber(FiberStateError::UnknownResumePoint(
            call.resume.0,
        )))?;
    if return_cursor
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
        FiberReturnContinuation::ProjectCallDefault { pending, .. } => {
            let default_value = value.ok_or_else(|| {
                VmError::Runtime("project-call default returned no attached value".to_owned())
            })?;
            if pending.state() != call.state {
                return Err(VmError::Runtime(
                    "project-call default resumed with a different pending callable state"
                        .to_owned(),
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
            let crate::plan::RuntimeCallableDefault::Body { function, .. } = default else {
                return Err(VmError::Runtime(
                    "project-call returned from an unbound generic default".to_owned(),
                ));
            };
            if returning_function != *function {
                return Err(VmError::Runtime(
                    "project-call returned from an unexpected default function".to_owned(),
                ));
            }
            let application = pending
                .complete_default(default_value)
                .map_err(|error| VmError::Runtime(error.to_string()))?;
            match application {
                RuntimeCallableApplication::Complete(result) => {
                    require_runtime_type(program, state.result, &result)?;
                    bind_pattern_owned(program, fiber, call.result_pattern, result)?;
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
                    fiber.push_call_frame_with_owned_continuation(
                        program, function, return_to, args,
                    )?;
                }
                RuntimeCallableApplication::AttachedDefault { .. } => {
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
            bind_pattern_owned(program, fiber, call.result_pattern, value)?;
            jump(fiber, resume.block);
        }
        FiberReturnContinuation::ApplyGroupDefault { .. } => unreachable!(
            "apply-group default return is handled before looking up a project-call site"
        ),
        FiberReturnContinuation::FormatOperand { .. }
        | FiberReturnContinuation::FormatDisplay { .. }
        | FiberReturnContinuation::InstructionCall { .. }
        | FiberReturnContinuation::ContextCallbackDefault { .. }
        | FiberReturnContinuation::ContextCallbackInvoke { .. } => {
            unreachable!("instruction return is handled by the fiber return continuation")
        }
        FiberReturnContinuation::Ordinary => {}
    }
    Ok(())
}

fn insert_project_call_source(
    sources: &mut BTreeSet<u32>,
    index: u32,
    operand_count: usize,
    label: &str,
) -> Result<(), VmError> {
    let index_usize = usize::try_from(index)
        .map_err(|_| VmError::Runtime(format!("{label} index exceeds usize")))?;
    if index_usize >= operand_count || !sources.insert(index) {
        return Err(VmError::Runtime(format!("{label} is absent or repeated")));
    }
    Ok(())
}

fn project_call_source_ref<'a>(
    operands: &[Vec<&'a RuntimeValue>],
    index: u32,
) -> Result<&'a RuntimeValue, VmError> {
    let index_usize = usize::try_from(index)
        .map_err(|_| VmError::Runtime("project-call source index exceeds usize".to_owned()))?;
    let mut remaining = index_usize;
    for operand in operands {
        if remaining < operand.len() {
            return Ok(operand[remaining]);
        }
        remaining -= operand.len();
    }
    Err(VmError::Runtime("project-call source is absent".to_owned()))
}

fn take_project_call_source_value(
    operands: &mut [Vec<Option<RuntimeValue>>],
    index: u32,
) -> Result<RuntimeValue, VmError> {
    let index_usize = usize::try_from(index)
        .map_err(|_| VmError::Runtime("project-call source index exceeds usize".to_owned()))?;
    let mut remaining = index_usize;
    for operand in operands {
        if remaining < operand.len() {
            return operand[remaining].take().ok_or_else(|| {
                VmError::Runtime("project-call source was consumed twice".to_owned())
            });
        }
        remaining -= operand.len();
    }
    Err(VmError::Runtime("project-call source is absent".to_owned()))
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

fn mutable_place_base(place: &AwbcMutablePlace) -> AwbcRegisterId {
    match place {
        AwbcMutablePlace::Local(register) => *register,
        AwbcMutablePlace::NominalField { base, .. } => *base,
    }
}

fn mutable_vec_sequence<'a>(
    frame: &'a mut FiberFrame,
    place: &AwbcMutablePlace,
    operation: &str,
) -> Result<&'a mut RuntimeSeq, VmError> {
    let base = mutable_place_base(place);
    let Some(receiver) = frame
        .registers
        .get_mut(base.index())
        .and_then(Option::as_mut)
    else {
        return Err(FiberStateError::RegisterOutOfBounds {
            register: base.0,
            layout: frame.layout.0,
        }
        .into());
    };
    match (place, receiver) {
        (AwbcMutablePlace::Local(_), RuntimeValue::Seq(sequence)) => Ok(sequence),
        (AwbcMutablePlace::NominalField { field, .. }, RuntimeValue::NominalRecord(record)) => {
            let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(*field as usize)
                .map_err(|_| {
                    VmError::Runtime(format!("{operation} has an invalid nominal field identity"))
                })?;
            record
                .sequence_field_mut(field)
                .map_err(|error| VmError::Runtime(error.to_string()))
        }
        (AwbcMutablePlace::Local(_), _) => Err(VmError::Runtime(format!(
            "{operation} expected a Vec value"
        ))),
        (AwbcMutablePlace::NominalField { .. }, _) => Err(VmError::Runtime(format!(
            "{operation} expected a nominal record receiver"
        ))),
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
    fiber: &mut FiberState,
    register_id: AwbcRegisterId,
) -> Result<FiberAwaitTarget, VmError> {
    let frame = fiber.active_frame()?;
    let runtime_type = program
        .frame_layouts
        .get(frame.layout.index())
        .and_then(|layout| layout.slots.get(register_id.index()))
        .and_then(|slot| program.runtime_types.get(slot.ty.index()))
        .ok_or_else(|| VmError::Runtime("await handle register has no runtime type".to_owned()))?;
    let value = fiber.active_frame_mut()?.take_register(register_id)?;
    match runtime_type.shape() {
        AwbcRuntimeTypeShape::Need(item_type) => match value {
            RuntimeValue::Need(id) if !id.0.is_empty() => Ok(FiberAwaitTarget::Need {
                id,
                item_type: *item_type,
                handle: register_id,
            }),
            value => Err(VmError::Runtime(format!(
                "NeedHandle register contained {}",
                runtime_value_label(&value)
            ))),
        },
        _ => Err(VmError::Runtime(
            "await register is not a Need handle".to_owned(),
        )),
    }
}

fn validate_host_call_values(
    program: &AwbcProgram,
    call: AwbcHostCallId,
    values: &[&RuntimeValue],
) -> Result<(), VmError> {
    let call = program
        .host_calls
        .get(call.index())
        .ok_or_else(|| VmError::Runtime(format!("AWBC host call {} is absent", call.0)))?;
    let signature = program
        .signatures
        .get(call.signature.index())
        .ok_or_else(|| VmError::Runtime("host call signature is absent".to_owned()))?;
    if values.len() != signature.params.len() {
        return Err(VmError::FunctionArgumentCount {
            expected: signature.params.len(),
            actual: values.len(),
        });
    }
    for (position, (value, expected)) in values.iter().zip(&signature.params).enumerate() {
        if !runtime_value_view_matches_type(program, value.view(), *expected, 0) {
            return Err(VmError::Runtime(format!(
                "host call argument {position} violates its sealed input type"
            )));
        }
        if !value.ownership().permits_copy() {
            return Err(VmError::Runtime(format!(
                "host call argument {position} contains an affine value"
            )));
        }
    }
    Ok(())
}

fn take_register_values(
    fiber: &mut FiberState,
    registers: &[AwbcRegisterId],
) -> Result<Vec<RuntimeValue>, VmError> {
    let mut seen = BTreeSet::new();
    for register_id in registers {
        if !seen.insert(*register_id) {
            return Err(VmError::Runtime(format!(
                "by-value AWBC operand register {} is listed more than once",
                register_id.0
            )));
        }
        register(fiber, *register_id)?;
    }
    let frame = fiber.active_frame_mut()?;
    registers
        .iter()
        .map(|register_id| frame.take_register(*register_id).map_err(VmError::from))
        .collect()
}

fn take_sequence_value(sequence: RuntimeSeq, index: usize) -> Result<RuntimeValue, VmError> {
    match sequence {
        RuntimeSeq::Values(mut values) => Ok(values.remove(index)),
        sequence if sequence.ownership().permits_copy() => Ok(sequence.value_at(index)),
        _ => Err(VmError::Runtime(
            "sequence storage cannot transfer an affine element".to_owned(),
        )),
    }
}

fn take_sequence_tail(sequence: RuntimeSeq, start: usize) -> Result<RuntimeSeq, VmError> {
    match sequence {
        RuntimeSeq::Values(mut values) => {
            let start = start.min(values.len());
            Ok(RuntimeSeq::Values(values.drain(start..).collect()))
        }
        sequence if sequence.ownership().permits_copy() => Ok(sequence.tail_from(start)),
        _ => Err(VmError::Runtime(
            "sequence storage cannot transfer an affine slice".to_owned(),
        )),
    }
}

fn jump(fiber: &mut FiberState, block: AwbcBlockId) {
    fiber.cursor.block = block;
    fiber.cursor.instruction_offset = 0;
}

pub(crate) fn terminal_exit(fiber: &mut FiberState) -> VmExit {
    match fiber.terminal.take() {
        Some(FiberTerminalValue::Returned(value)) => {
            if let Some(value) = value.as_ref() {
                fiber.return_summary = Some(runtime_value_label(value));
            }
            fiber.terminal = Some(FiberTerminalValue::Returned(None));
            VmExit::Returned(value)
        }
        Some(FiberTerminalValue::DialogueResultSelected(value)) => {
            fiber.return_summary = None;
            fiber.terminal = Some(FiberTerminalValue::Returned(None));
            VmExit::DialogueResultSelected(value)
        }
        Some(FiberTerminalValue::Cancelled) => {
            fiber.terminal = Some(FiberTerminalValue::Cancelled);
            VmExit::Cancelled
        }
        Some(FiberTerminalValue::Trapped(trap)) => {
            fiber.terminal = Some(FiberTerminalValue::Trapped(trap.clone()));
            VmExit::Trapped(trap)
        }
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
        AwbcConstant::Color(value) => Ok(RuntimeValue::Color(*value)),
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
    test_pattern_view(program, pattern, value.view(), 0)
}

fn test_pattern_view(
    program: &AwbcProgram,
    pattern: AwbcPatternId,
    value: RuntimeValueView<'_>,
    depth: usize,
) -> Result<bool, VmError> {
    if depth > 1024 {
        return Err(VmError::Runtime("pattern depth exceeded".to_owned()));
    }
    let pattern = program
        .patterns
        .get(pattern.index())
        .ok_or(VmError::MissingPattern(pattern))?;
    Ok(match pattern {
        AwbcPattern::Bind { expected, .. } => expected
            .is_none_or(|expected| runtime_value_view_matches_type(program, value, expected, 0)),
        AwbcPattern::Discard => true,
        AwbcPattern::Literal(id) => {
            runtime_value_views_equal(constant_value(program, *id)?.view(), value)
        }
        AwbcPattern::Entity(expected) => {
            matches!(value, RuntimeValueView::Scalar(RuntimeScalarView::EntityRef(actual)) if actual == expected)
        }
        AwbcPattern::Tuple(patterns) => match value {
            RuntimeValueView::Tuple(values) if values.len() == patterns.len() => {
                let mut matched = true;
                for (index, pattern) in patterns.iter().enumerate() {
                    let Some(value) = values.get(index) else {
                        matched = false;
                        break;
                    };
                    if !test_pattern_view(program, *pattern, value, depth + 1)? {
                        matched = false;
                        break;
                    }
                }
                matched
            }
            _ => false,
        },
        AwbcPattern::Record { ty, fields, rest } => {
            let owner_matches =
                ty.is_none_or(|ty| runtime_value_view_matches_type(program, value, ty, 0));
            owner_matches
                && match value {
                    RuntimeValueView::Record(values) => {
                        if !rest.accepts_len(fields.len(), values.len()) {
                            false
                        } else {
                            let mut matched = true;
                            for field in fields {
                                let Some((identity, _, value)) = values.get(field.field as usize)
                                else {
                                    matched = false;
                                    break;
                                };
                                if identity.zero_based() != field.field
                                    || !test_pattern_view(program, field.pattern, value, depth + 1)?
                                {
                                    matched = false;
                                    break;
                                }
                            }
                            matched
                        }
                    }
                    RuntimeValueView::NominalRecord(record) => {
                        if !rest.accepts_len(fields.len(), record.fields().len()) {
                            false
                        } else {
                            let mut matched = true;
                            for field in fields {
                                let Some(value) = record.fields().get(field.field as usize) else {
                                    matched = false;
                                    break;
                                };
                                if !test_pattern_view(
                                    program,
                                    field.pattern,
                                    value.view(),
                                    depth + 1,
                                )? {
                                    matched = false;
                                    break;
                                }
                            }
                            matched
                        }
                    }
                    _ => false,
                }
        }
        AwbcPattern::Sequence { items, rest } => match value {
            RuntimeValueView::Sequence(sequence)
                if rest.accepts_len(items.len(), sequence.len()) =>
            {
                let mut matched = true;
                for (index, pattern) in items.iter().enumerate() {
                    let Some(value) = sequence.value_view(index) else {
                        matched = false;
                        break;
                    };
                    if !test_pattern_view(program, *pattern, value, depth + 1)? {
                        matched = false;
                        break;
                    }
                }
                matched
            }
            _ => false,
        },
        AwbcPattern::Variant {
            ty,
            case,
            case_name,
            payload,
        } => {
            let case_name = string(program, *case_name)?;
            if !runtime_value_view_matches_type(program, value, *ty, 0) {
                false
            } else if let RuntimeValueView::Variant {
                ordinal,
                name,
                payload: actual,
                ..
            } = value
            {
                if *case != ordinal || case_name != name {
                    false
                } else {
                    match (payload, actual) {
                        (None, None) => true,
                        (Some(pattern), Some(value)) => {
                            test_pattern_view(program, *pattern, value.view(), depth + 1)?
                        }
                        _ => false,
                    }
                }
            } else {
                false
            }
        }
        AwbcPattern::Whole { inner, .. } => test_pattern_view(program, *inner, value, depth + 1)?,
    })
}

fn runtime_value_views_equal(left: RuntimeValueView<'_>, right: RuntimeValueView<'_>) -> bool {
    match (left, right) {
        (RuntimeValueView::Scalar(left), RuntimeValueView::Scalar(right)) => left == right,
        (RuntimeValueView::Tuple(left), RuntimeValueView::Tuple(right)) => {
            left.len() == right.len()
                && (0..left.len()).all(|index| match (left.get(index), right.get(index)) {
                    (Some(left), Some(right)) => runtime_value_views_equal(left, right),
                    _ => false,
                })
        }
        (RuntimeValueView::Record(left), RuntimeValueView::Record(right)) => {
            left.len() == right.len()
                && (0..left.len()).all(|index| match (left.get(index), right.get(index)) {
                    (Some((left_id, left_name, left)), Some((right_id, right_name, right))) => {
                        left_id == right_id
                            && left_name == right_name
                            && runtime_value_views_equal(left, right)
                    }
                    _ => false,
                })
        }
        (RuntimeValueView::Sequence(left), RuntimeValueView::Sequence(right)) => {
            left.len() == right.len()
                && (0..left.len()).all(|index| {
                    match (left.value_view(index), right.value_view(index)) {
                        (Some(left), Some(right)) => runtime_value_views_equal(left, right),
                        _ => false,
                    }
                })
        }
        (RuntimeValueView::NominalRecord(left), RuntimeValueView::NominalRecord(right)) => {
            left == right
        }
        (RuntimeValueView::Opaque(left), RuntimeValueView::Opaque(right)) => left == right,
        (RuntimeValueView::Reduction(left), RuntimeValueView::Reduction(right)) => left == right,
        (RuntimeValueView::Agent(left), RuntimeValueView::Agent(right)) => left == right,
        (
            RuntimeValueView::Variant {
                owner: left_owner,
                ordinal: left_ordinal,
                name: left_name,
                payload: left_payload,
            },
            RuntimeValueView::Variant {
                owner: right_owner,
                ordinal: right_ordinal,
                name: right_name,
                payload: right_payload,
            },
        ) => {
            left_owner == right_owner
                && left_ordinal == right_ordinal
                && left_name == right_name
                && match (left_payload, right_payload) {
                    (None, None) => true,
                    (Some(left), Some(right)) => {
                        runtime_value_views_equal(left.view(), right.view())
                    }
                    _ => false,
                }
        }
        (RuntimeValueView::RuntimeOnly(left), RuntimeValueView::RuntimeOnly(right)) => {
            left == right
        }
        _ => false,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "borrowed AWBC type admission mirrors the single runtime type table without materializing affine rows"
)]
pub(crate) fn runtime_value_view_matches_type(
    program: &AwbcProgram,
    value: RuntimeValueView<'_>,
    ty: AwbcTypeId,
    depth: usize,
) -> bool {
    if depth > 64 {
        return false;
    }
    let Some(ty_row) = program.runtime_types.get(ty.index()) else {
        return false;
    };
    match (value, ty_row.shape()) {
        (RuntimeValueView::Reduction(value), AwbcRuntimeTypeShape::Opaque { arguments, .. }) => {
            program.opaque_owner(ty).ok().flatten().is_some_and(|owner| {
                owner == *value.owner()
                    && arguments.len() == 1
                    && runtime_value_view_matches_type(
                        program,
                        value.state().view(),
                        arguments[0],
                        depth + 1,
                    )
            })
        }
        (RuntimeValueView::RuntimeOnly(RuntimeValue::Need(need)), AwbcRuntimeTypeShape::Need(_)) => {
            !need.0.is_empty()
        }
        (_, AwbcRuntimeTypeShape::Dynamic)
        | (RuntimeValueView::Scalar(RuntimeScalarView::Unit), AwbcRuntimeTypeShape::Unit)
        | (RuntimeValueView::Scalar(RuntimeScalarView::Bool(_)), AwbcRuntimeTypeShape::Bool)
        | (RuntimeValueView::Scalar(RuntimeScalarView::F32(_)), AwbcRuntimeTypeShape::F32)
        | (RuntimeValueView::Scalar(RuntimeScalarView::F64(_)), AwbcRuntimeTypeShape::F64)
        | (RuntimeValueView::Scalar(RuntimeScalarView::String(_)), AwbcRuntimeTypeShape::String)
        | (RuntimeValueView::Scalar(RuntimeScalarView::String(_)), AwbcRuntimeTypeShape::Task(_))
        | (RuntimeValueView::Scalar(RuntimeScalarView::Color(_)), AwbcRuntimeTypeShape::Color)
        | (RuntimeValueView::Scalar(RuntimeScalarView::Char(_)), AwbcRuntimeTypeShape::Char)
        | (RuntimeValueView::Scalar(RuntimeScalarView::Duration(_)), AwbcRuntimeTypeShape::Duration)
        | (RuntimeValueView::Scalar(RuntimeScalarView::Progress(_)), AwbcRuntimeTypeShape::Progress)
        | (RuntimeValueView::Scalar(RuntimeScalarView::EntityRef(_)), AwbcRuntimeTypeShape::EntityRef)
        | (RuntimeValueView::RuntimeOnly(RuntimeValue::MatrixF32(_)), AwbcRuntimeTypeShape::MatrixF32)
        | (RuntimeValueView::RuntimeOnly(RuntimeValue::MatrixF64(_)), AwbcRuntimeTypeShape::MatrixF64)
        | (RuntimeValueView::RuntimeOnly(RuntimeValue::TensorF32(_)), AwbcRuntimeTypeShape::TensorF32)
        | (RuntimeValueView::RuntimeOnly(RuntimeValue::TensorF64(_)), AwbcRuntimeTypeShape::TensorF64) => true,
        (RuntimeValueView::RuntimeOnly(RuntimeValue::Callable(value)), AwbcRuntimeTypeShape::Function { .. }) => {
            matches!(value.owner(), RuntimeProgramOwner::Awbc(owner) if std::ptr::eq(owner.as_ref(), program))
                && value.function_type().ok() == Some(ty_row.semantic_identity())
                && value.validate_retained().is_ok()
        }
        (RuntimeValueView::Agent(value), AwbcRuntimeTypeShape::Agent(expected)) => {
            value.operational_type() == expected.operational_type()
        }
        (RuntimeValueView::Record(_), AwbcRuntimeTypeShape::Agent(expected)) => {
            expected.operational_type().accepts_protocol_record()
        }
        (RuntimeValueView::Sequence(values), AwbcRuntimeTypeShape::Bytes) => {
            (0..values.len()).all(|index| matches!(values.value_view(index), Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(value))) if value.width() == crate::value::RuntimeUnsignedIntWidth::U8))
        }
        (RuntimeValueView::Scalar(RuntimeScalarView::Int(value)), AwbcRuntimeTypeShape::Int(kind)) => {
            signed_kind(value) == *kind
        }
        (RuntimeValueView::Scalar(RuntimeScalarView::UInt(value)), AwbcRuntimeTypeShape::UInt(kind)) => {
            unsigned_kind(value) == *kind
        }
        (RuntimeValueView::Opaque(value), AwbcRuntimeTypeShape::Opaque { .. }) => program
            .opaque_owner(ty)
            .ok()
            .flatten()
            .is_some_and(|owner| owner.accepts_opaque_value(value)),
        (RuntimeValueView::Tuple(values), AwbcRuntimeTypeShape::Tuple(types)) => {
            values.len() == types.len()
                && types.iter().enumerate().all(|(index, ty)| {
                    values.get(index).is_some_and(|value| {
                        runtime_value_view_matches_type(program, value, *ty, depth + 1)
                    })
                })
        }
        (RuntimeValueView::Sequence(values), AwbcRuntimeTypeShape::Sequence { item, .. }) => {
            (0..values.len()).all(|index| values.value_view(index).is_some_and(|value| {
                runtime_value_view_matches_type(program, value, *item, depth + 1)
            }))
        }
        (RuntimeValueView::Sequence(values), AwbcRuntimeTypeShape::Array { item, length }) => {
            length.constant().and_then(|length| usize::try_from(length).ok()) == Some(values.len())
                && (0..values.len()).all(|index| values.value_view(index).is_some_and(|value| {
                    runtime_value_view_matches_type(program, value, *item, depth + 1)
                }))
        }
        (RuntimeValueView::Record(values), AwbcRuntimeTypeShape::Record { fields, .. }) => {
            values.len() == fields.len()
                && fields.iter().enumerate().all(|(index, field)| {
                    values.get(index).is_some_and(|(_, _, value)| {
                        runtime_value_view_matches_type(program, value, field.ty, depth + 1)
                    })
                })
        }
        (
            RuntimeValueView::Variant { owner, ordinal, name, payload },
            AwbcRuntimeTypeShape::Variant { owner: expected_owner, cases, .. },
        ) => {
            runtime_variant_identity(program, ty_row.semantic_identity(), expected_owner).as_ref()
                == Some(owner)
                && usize::try_from(ordinal).ok().and_then(|ordinal| cases.get(ordinal)).is_some_and(|case| {
                    program.strings.get(case.name.index()).is_some_and(|case_name| {
                        case_name == name && match (case.payload, payload) {
                            (None, None) => true,
                            (Some(expected), Some(value)) => runtime_value_view_matches_type(program, value.view(), expected, depth + 1),
                            _ => false,
                        }
                    })
                })
        }
        (value, AwbcRuntimeTypeShape::Choice(alternatives)) => alternatives
            .iter()
            .any(|alternative| runtime_value_view_matches_type(program, value, *alternative, depth + 1)),
        (RuntimeValueView::NominalRecord(record), AwbcRuntimeTypeShape::Nominal { public_id, layout, .. }) => {
            program.strings.get(public_id.index()).is_some_and(|expected| record.type_id().as_str() == expected)
                && record.layout().as_bytes() == layout
        }
        (RuntimeValueView::NominalRecord(record), AwbcRuntimeTypeShape::NominalRecord { .. }) => program
            .nominal_record_layout(ty)
            .ok()
            .flatten()
            .is_some_and(|layout| record.validate_against_layout(&layout).is_ok()),
        (RuntimeValueView::RuntimeOnly(RuntimeValue::Range(range)), AwbcRuntimeTypeShape::Range(item)) => {
            use crate::value::RuntimeRange;
            match range {
                RuntimeRange::Int { start, end, .. } => {
                    (start.is_some() || end.is_some())
                        && start.is_none_or(|value| {
                            runtime_value_view_matches_type(
                                program,
                                RuntimeValueView::Scalar(RuntimeScalarView::Int(value)),
                                *item,
                                depth + 1,
                            )
                        })
                        && end.is_none_or(|value| {
                            runtime_value_view_matches_type(
                                program,
                                RuntimeValueView::Scalar(RuntimeScalarView::Int(value)),
                                *item,
                                depth + 1,
                            )
                        })
                }
                RuntimeRange::UInt { start, end, .. } => {
                    (start.is_some() || end.is_some())
                        && start.is_none_or(|value| {
                            runtime_value_view_matches_type(
                                program,
                                RuntimeValueView::Scalar(RuntimeScalarView::UInt(value)),
                                *item,
                                depth + 1,
                            )
                        })
                        && end.is_none_or(|value| {
                            runtime_value_view_matches_type(
                                program,
                                RuntimeValueView::Scalar(RuntimeScalarView::UInt(value)),
                                *item,
                                depth + 1,
                            )
                        })
                }
            }
        }
        (RuntimeValueView::RuntimeOnly(RuntimeValue::Iterator(_)), AwbcRuntimeTypeShape::Iterator { .. }) => true,
        _ => false,
    }
}

fn signed_kind(value: crate::value::RuntimeInt) -> AwbcSignedIntKind {
    match value.width() {
        crate::value::RuntimeSignedIntWidth::I8 => AwbcSignedIntKind::I8,
        crate::value::RuntimeSignedIntWidth::I16 => AwbcSignedIntKind::I16,
        crate::value::RuntimeSignedIntWidth::I32 => AwbcSignedIntKind::I32,
        crate::value::RuntimeSignedIntWidth::I64 => AwbcSignedIntKind::I64,
        crate::value::RuntimeSignedIntWidth::I128 => AwbcSignedIntKind::I128,
        crate::value::RuntimeSignedIntWidth::ISize => AwbcSignedIntKind::ISize,
    }
}

fn unsigned_kind(value: crate::value::RuntimeUInt) -> AwbcUnsignedIntKind {
    match value.width() {
        crate::value::RuntimeUnsignedIntWidth::U8 => AwbcUnsignedIntKind::U8,
        crate::value::RuntimeUnsignedIntWidth::U16 => AwbcUnsignedIntKind::U16,
        crate::value::RuntimeUnsignedIntWidth::U32 => AwbcUnsignedIntKind::U32,
        crate::value::RuntimeUnsignedIntWidth::U64 => AwbcUnsignedIntKind::U64,
        crate::value::RuntimeUnsignedIntWidth::U128 => AwbcUnsignedIntKind::U128,
        crate::value::RuntimeUnsignedIntWidth::USize => AwbcUnsignedIntKind::USize,
    }
}

pub(crate) fn bind_pattern_owned(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    pattern: AwbcPatternId,
    value: RuntimeValue,
) -> Result<(), VmError> {
    let prepared = prepare_pattern_binding(program, fiber, pattern, &value)?;
    bind_pattern_owned_prepared(program, fiber, prepared, value);
    Ok(())
}

/// A single-use proof that an owned pattern bind has passed every fallible
/// match, projection, duplication, and destination-slot check for one frame.
pub(crate) struct PreparedPatternBinding {
    pattern: AwbcPatternId,
    frame: crate::runtime_id::RuntimeFrameInstanceId,
    layout: crate::awbc::schema::AwbcFrameLayoutId,
    cursor: FiberCursor,
    registers: Box<[AwbcRegisterId]>,
}

impl PreparedPatternBinding {
    pub(crate) fn registers(&self) -> &[AwbcRegisterId] {
        &self.registers
    }
}

pub(crate) fn prepare_pattern_binding(
    program: &AwbcProgram,
    fiber: &FiberState,
    pattern: AwbcPatternId,
    value: &RuntimeValue,
) -> Result<PreparedPatternBinding, VmError> {
    if !test_pattern(program, pattern, value)? {
        return Err(VmError::PatternMismatch);
    }
    let frame = fiber.active_frame()?;
    let layout = program
        .frame_layouts
        .get(frame.layout.index())
        .ok_or(FiberStateError::UnknownFrameLayout(frame.layout.0))?;
    let mut registers = Vec::new();
    visit_pattern_bindings_view(program, pattern, value.view(), 0, &mut |register, view| {
        let slot =
            layout
                .slots
                .get(register.index())
                .ok_or(FiberStateError::RegisterOutOfBounds {
                    register: register.0,
                    layout: frame.layout.0,
                })?;
        if !runtime_value_view_matches_type(program, view, slot.ty, 0) {
            return Err(VmError::Runtime(format!(
                "pattern target register {} rejects the projected value type",
                register.0
            )));
        }
        registers.push(register);
        Ok(())
    })?;
    registers.sort_unstable();
    let original_len = registers.len();
    registers.dedup();
    if registers.len() != original_len {
        return Err(VmError::Runtime(
            "AWBC pattern binds one register more than once".to_owned(),
        ));
    }
    Ok(PreparedPatternBinding {
        pattern,
        frame: frame.instance,
        layout: frame.layout,
        cursor: fiber.cursor,
        registers: registers.into_boxed_slice(),
    })
}

/// Commits a value using a consumed preflight token. All indexing and pattern
/// operations here were proven against the same frame before the source owner
/// was moved.
pub(crate) fn bind_pattern_owned_prepared(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    prepared: PreparedPatternBinding,
    value: RuntimeValue,
) {
    assert_eq!(fiber.cursor, prepared.cursor);
    {
        let frame = fiber
            .active_frame()
            .expect("prepared pattern bind retains its active frame");
        assert_eq!(frame.instance, prepared.frame);
        assert_eq!(frame.layout, prepared.layout);
    }
    bind_tested_pattern_owned(program, fiber, prepared.pattern, value)
        .expect("prepared pattern bind has no remaining validation failures");
}

/// Binds from a borrowed external payload only when that payload is explicitly
/// proven copyable. Bytecode and owned product paths should use
/// `bind_pattern_owned` so affine values move out of their source slot.
pub(crate) fn bind_pattern_copyable(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    pattern: AwbcPatternId,
    value: &RuntimeValue,
) -> Result<(), VmError> {
    if !value.ownership().permits_copy() {
        return Err(VmError::Runtime(
            "borrowed pattern payload cannot duplicate an affine value".to_owned(),
        ));
    }
    bind_pattern_owned(program, fiber, pattern, value.clone())
}

/// Applies a pattern graph only after the complete root has matched.
///
/// Keeping all writes behind the root pretest makes binding atomic with
/// respect to ordinary mismatch: no child register can be written before a
/// later exact-length, literal, or type predicate fails.
fn bind_tested_pattern_owned(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    pattern: AwbcPatternId,
    value: RuntimeValue,
) -> Result<(), VmError> {
    let pattern_record = program
        .patterns
        .get(pattern.index())
        .ok_or(VmError::MissingPattern(pattern))?
        .clone();
    match pattern_record {
        AwbcPattern::Bind { target, .. } => {
            fiber.active_frame_mut()?.set_register(target, value)?;
        }
        AwbcPattern::Whole { target, inner } => {
            let whole = if pattern_has_binding(program, inner, 0)? {
                if !value.ownership().permits_copy() {
                    return Err(VmError::Runtime(
                        "Whole pattern would duplicate an affine value".to_owned(),
                    ));
                }
                let inner_value = value.clone();
                bind_tested_pattern_owned(program, fiber, inner, inner_value)?;
                value
            } else {
                value
            };
            fiber.active_frame_mut()?.set_register(target, whole)?;
        }
        AwbcPattern::Tuple(children) => {
            if let RuntimeValue::Tuple(values) = value {
                for (child, value) in children.into_iter().zip(values) {
                    if pattern_has_binding(program, child, 0)? {
                        bind_tested_pattern_owned(program, fiber, child, value)?;
                    }
                }
            }
        }
        AwbcPattern::Sequence { items, rest } => {
            if let RuntimeValue::Seq(sequence) = value {
                let mut values = sequence.into_values().into_iter();
                for child in items {
                    let value = values.next().ok_or_else(|| {
                        VmError::Runtime("sequence pattern value is absent".to_owned())
                    })?;
                    if pattern_has_binding(program, child, 0)? {
                        bind_tested_pattern_owned(program, fiber, child, value)?;
                    }
                }
                if let AwbcPatternRest::Bind(rest) = rest {
                    let tail = values.collect();
                    fiber
                        .active_frame_mut()?
                        .set_register(rest, runtime_sequence_from_literal_values(tail))?;
                }
            }
        }
        AwbcPattern::Record { fields, rest, .. } => {
            let has_field_bindings = fields.iter().try_fold(false, |has_bindings, field| {
                Ok::<_, VmError>(has_bindings || pattern_has_binding(program, field.pattern, 0)?)
            })?;
            match rest {
                AwbcPatternRest::Bind(rest) if !has_field_bindings => {
                    fiber.active_frame_mut()?.set_register(rest, value)?;
                }
                AwbcPatternRest::Bind(rest) => {
                    if !value.ownership().permits_copy() {
                        return Err(VmError::Runtime(
                            "record rest pattern would duplicate an affine value".to_owned(),
                        ));
                    }
                    let retained = value.clone();
                    let mut values = into_record_field_values(value)?;
                    bind_record_pattern_fields(program, fiber, fields, &mut values)?;
                    fiber.active_frame_mut()?.set_register(rest, retained)?;
                }
                AwbcPatternRest::Exact | AwbcPatternRest::Ignore => {
                    let mut values = into_record_field_values(value)?;
                    bind_record_pattern_fields(program, fiber, fields, &mut values)?;
                }
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
                if pattern_has_binding(program, pattern, 0)? {
                    bind_tested_pattern_owned(program, fiber, pattern, *value)?;
                }
            }
        }
        AwbcPattern::Discard | AwbcPattern::Literal(_) | AwbcPattern::Entity(_) => {}
    }
    Ok(())
}

fn pattern_has_binding(
    program: &AwbcProgram,
    pattern: AwbcPatternId,
    depth: usize,
) -> Result<bool, VmError> {
    if depth > 1024 {
        return Err(VmError::Runtime("pattern depth exceeded".to_owned()));
    }
    let pattern = program
        .patterns
        .get(pattern.index())
        .ok_or(VmError::MissingPattern(pattern))?;
    match pattern {
        AwbcPattern::Bind { .. } => Ok(true),
        AwbcPattern::Tuple(items) => items.iter().try_fold(false, |found, item| {
            Ok::<_, VmError>(found || pattern_has_binding(program, *item, depth + 1)?)
        }),
        AwbcPattern::Sequence { items, rest } => {
            if matches!(rest, AwbcPatternRest::Bind(_)) {
                Ok(true)
            } else {
                items.iter().try_fold(false, |found, item| {
                    Ok::<_, VmError>(found || pattern_has_binding(program, *item, depth + 1)?)
                })
            }
        }
        AwbcPattern::Record { fields, rest, .. } => {
            if matches!(rest, AwbcPatternRest::Bind(_)) {
                Ok(true)
            } else {
                fields.iter().try_fold(false, |found, field| {
                    Ok::<_, VmError>(
                        found || pattern_has_binding(program, field.pattern, depth + 1)?,
                    )
                })
            }
        }
        AwbcPattern::Variant {
            payload: Some(payload),
            ..
        } => pattern_has_binding(program, *payload, depth + 1),
        AwbcPattern::Whole { .. } => Ok(true),
        AwbcPattern::Discard
        | AwbcPattern::Literal(_)
        | AwbcPattern::Entity(_)
        | AwbcPattern::Variant { payload: None, .. } => Ok(false),
    }
}

/// Maps each affine handle token in a borrowed value to the one register that
/// the already-tested pattern will bind it into. This is used by Product's
/// ownership preflight before the source value leaves line custody.
pub(crate) fn pattern_handle_destinations(
    program: &AwbcProgram,
    fiber: &FiberState,
    pattern: AwbcPatternId,
    value: &RuntimeValue,
    token: &crate::runtime_id::RuntimeLineHandleToken,
) -> Result<Vec<AwbcRegisterId>, VmError> {
    let prepared = prepare_pattern_binding(program, fiber, pattern, value)?;
    let mut destinations = Vec::new();
    visit_pattern_bindings_view(program, pattern, value.view(), 0, &mut |register, view| {
        if view
            .contains_line_handle(token)
            .map_err(|error| VmError::Runtime(error.to_string()))?
        {
            destinations.push(register);
        }
        Ok(())
    })?;
    let admitted = prepared.registers.iter().copied().collect::<BTreeSet<_>>();
    if destinations
        .iter()
        .any(|destination| !admitted.contains(destination))
    {
        return Err(VmError::Runtime(
            "pattern handle preview referenced an unbound register".to_owned(),
        ));
    }
    destinations.sort_unstable();
    destinations.dedup();
    if destinations.len() > 1 {
        return Err(VmError::Runtime(
            "AWBC result pattern duplicates one affine line handle into multiple registers"
                .to_owned(),
        ));
    }
    Ok(destinations)
}

/// Preflights every register that `bind_pattern_owned` will write, including
/// target-slot type admission and duplication constraints, without moving any
/// part of the borrowed result value.
pub(crate) fn pattern_binding_registers(
    program: &AwbcProgram,
    fiber: &FiberState,
    pattern: AwbcPatternId,
    value: &RuntimeValue,
) -> Result<Vec<AwbcRegisterId>, VmError> {
    prepare_pattern_binding(program, fiber, pattern, value)
        .map(|prepared| prepared.registers.into_vec())
}

fn visit_pattern_bindings_view(
    program: &AwbcProgram,
    pattern: AwbcPatternId,
    value: RuntimeValueView<'_>,
    depth: usize,
    visitor: &mut impl FnMut(AwbcRegisterId, RuntimeValueView<'_>) -> Result<(), VmError>,
) -> Result<(), VmError> {
    if depth > 1024 {
        return Err(VmError::Runtime("pattern depth exceeded".to_owned()));
    }
    let row = program
        .patterns
        .get(pattern.index())
        .ok_or(VmError::MissingPattern(pattern))?;
    match row {
        AwbcPattern::Bind { target, .. } => visitor(*target, value),
        AwbcPattern::Discard | AwbcPattern::Literal(_) | AwbcPattern::Entity(_) => Ok(()),
        AwbcPattern::Tuple(patterns) => {
            let RuntimeValueView::Tuple(values) = value else {
                return Err(VmError::PatternMismatch);
            };
            for (index, pattern) in patterns.iter().enumerate() {
                let value = values.get(index).ok_or(VmError::PatternMismatch)?;
                visit_pattern_bindings_view(program, *pattern, value, depth + 1, visitor)?;
            }
            Ok(())
        }
        AwbcPattern::Record { fields, rest, .. } => {
            let has_field_bindings = fields.iter().try_fold(false, |has_bindings, field| {
                Ok::<_, VmError>(has_bindings || pattern_has_binding(program, field.pattern, 0)?)
            })?;
            if let AwbcPatternRest::Bind(register) = rest {
                if has_field_bindings && !value.ownership().permits_copy() {
                    return Err(VmError::Runtime(
                        "record rest pattern would duplicate an affine value".to_owned(),
                    ));
                }
                if !has_field_bindings {
                    return visitor(*register, value);
                }
                visitor(*register, value)?;
            }
            let RuntimeValueView::Record(values) = value else {
                return if matches!(value, RuntimeValueView::NominalRecord(_)) {
                    let RuntimeValueView::NominalRecord(record) = value else {
                        unreachable!()
                    };
                    for field in fields {
                        let nested = record
                            .fields()
                            .get(field.field as usize)
                            .ok_or(VmError::PatternMismatch)?
                            .view();
                        visit_pattern_bindings_view(
                            program,
                            field.pattern,
                            nested,
                            depth + 1,
                            visitor,
                        )?;
                    }
                    Ok(())
                } else {
                    Err(VmError::PatternMismatch)
                };
            };
            for field in fields {
                let (identity, _, nested) = values
                    .get(field.field as usize)
                    .ok_or(VmError::PatternMismatch)?;
                if identity.zero_based() != field.field {
                    return Err(VmError::PatternMismatch);
                }
                visit_pattern_bindings_view(program, field.pattern, nested, depth + 1, visitor)?;
            }
            Ok(())
        }
        AwbcPattern::Sequence { items, rest } => {
            let RuntimeValueView::Sequence(values) = value else {
                return Err(VmError::PatternMismatch);
            };
            for (index, pattern) in items.iter().enumerate() {
                let nested = values.value_view(index).ok_or(VmError::PatternMismatch)?;
                visit_pattern_bindings_view(program, *pattern, nested, depth + 1, visitor)?;
            }
            if let AwbcPatternRest::Bind(register) = rest {
                for index in items.len()..values.len() {
                    let nested = values.value_view(index).ok_or(VmError::PatternMismatch)?;
                    visitor(*register, nested)?;
                }
            }
            Ok(())
        }
        AwbcPattern::Variant { payload, .. } => {
            if let (
                Some(pattern),
                RuntimeValueView::Variant {
                    payload: Some(value),
                    ..
                },
            ) = (payload, value)
            {
                visit_pattern_bindings_view(program, *pattern, value.view(), depth + 1, visitor)?;
            }
            Ok(())
        }
        AwbcPattern::Whole { target, inner } => {
            if pattern_has_binding(program, *inner, 0)? {
                if !value.ownership().permits_copy() {
                    return Err(VmError::Runtime(
                        "Whole pattern would duplicate an affine value".to_owned(),
                    ));
                }
                visit_pattern_bindings_view(program, *inner, value, depth + 1, visitor)?;
            }
            visitor(*target, value)
        }
    }
}

/// Checks the exact entry-time deep-Copy facts sealed by the selected
/// function's positional ABI before any argument owner moves into a frame.
pub(crate) fn validate_function_input_ownership_values(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    values: &[&RuntimeValue],
) -> Result<(), VmError> {
    let function_record = program
        .functions
        .get(function.index())
        .ok_or(VmError::MissingFunction(function))?;
    let signature = program
        .signatures
        .get(function_record.signature.index())
        .ok_or_else(|| VmError::Runtime("function signature is absent".to_owned()))?;
    if values.len() != signature.params.len()
        || function_record.input_ownership.len() != signature.params.len()
    {
        return Err(VmError::FunctionArgumentCount {
            expected: signature.params.len(),
            actual: values.len(),
        });
    }
    for (position, (row, value)) in function_record
        .input_ownership
        .iter()
        .zip(values)
        .enumerate()
    {
        if row.requirement == RuntimeFunctionInputOwnershipRequirement::Unrestricted
            && !value.ownership().permits_copy()
        {
            return Err(VmError::Runtime(format!(
                "function input {position} requires a deep Copy value"
            )));
        }
        let required = row
            .unrestricted_bindings
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if required.len() != row.unrestricted_bindings.len() {
            return Err(VmError::Runtime(format!(
                "function input {position} repeats an unrestricted binding coordinate"
            )));
        }
        if required.is_empty() {
            continue;
        }
        let pattern = row.pattern.ok_or_else(|| {
            VmError::Runtime(format!(
                "function input {position} has unrestricted bindings without a pattern"
            ))
        })?;
        let mut observed = BTreeSet::new();
        visit_pattern_bindings_view(
            program,
            pattern,
            value.view(),
            0,
            &mut |register, projected| {
                if required.contains(&register) {
                    if !projected.ownership().permits_copy() {
                        return Err(VmError::Runtime(format!(
                            "function input {position} binding {register:?} requires a deep Copy value"
                        )));
                    }
                    observed.insert(register);
                }
                Ok(())
            },
        )?;
        if observed != required {
            return Err(VmError::Runtime(format!(
                "function input {position} unrestricted bindings disagree with its projected pattern"
            )));
        }
    }
    Ok(())
}

fn into_record_field_values(value: RuntimeValue) -> Result<Vec<Option<RuntimeValue>>, VmError> {
    match value {
        RuntimeValue::Record(fields) => Ok(fields
            .into_iter()
            .map(|field| Some(field.into_value()))
            .collect()),
        RuntimeValue::NominalRecord(record) => {
            Ok(record.into_fields().into_iter().map(Some).collect())
        }
        _ => Err(VmError::Runtime(
            "record pattern tested a non-record value".to_owned(),
        )),
    }
}

fn bind_record_pattern_fields(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    fields: Vec<super::schema::AwbcRecordPatternField>,
    values: &mut [Option<RuntimeValue>],
) -> Result<(), VmError> {
    for field in fields {
        let value = values
            .get_mut(field.field as usize)
            .and_then(Option::take)
            .ok_or_else(|| VmError::Runtime("record pattern field is absent".to_owned()))?;
        if pattern_has_binding(program, field.pattern, 0)? {
            bind_tested_pattern_owned(program, fiber, field.pattern, value)?;
        }
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

fn format_primary_kind(
    program: &AwbcProgram,
    function: AwbcFunctionId,
) -> Result<crate::value::RuntimeFormatPrimaryKind, VmError> {
    let function = program
        .functions
        .get(function.index())
        .ok_or(VmError::MissingFunction(function))?;
    let signature = program
        .signatures
        .get(function.signature.index())
        .ok_or_else(|| VmError::Runtime("formatter operand signature is absent".to_owned()))?;
    let result = signature
        .result
        .ok_or_else(|| VmError::Runtime("formatter value operand has no result".to_owned()))?;
    format_primary_kind_for_type(program, result)
}

fn format_primary_kind_for_type(
    program: &AwbcProgram,
    result: AwbcTypeId,
) -> Result<crate::value::RuntimeFormatPrimaryKind, VmError> {
    if let Some(item) = program.builtin_variant_payload_item(
        result,
        crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome,
    ) {
        let item = program
            .runtime_types
            .get(item.index())
            .ok_or(VmError::MissingType(item))?;
        return Ok(crate::value::RuntimeFormatPrimaryKind::OptionScalar(
            item.semantic_identity(),
        ));
    }
    let ty = program
        .runtime_types
        .get(result.index())
        .ok_or(VmError::MissingType(result))?;
    if ty.semantic_identity()
        == crate::value::RuntimeDialogueOpaqueRole::Content.semantic_identity()
    {
        Ok(crate::value::RuntimeFormatPrimaryKind::Content)
    } else {
        Ok(crate::value::RuntimeFormatPrimaryKind::Scalar(
            ty.semantic_identity(),
        ))
    }
}

impl VmError {
    fn runtime_trap_code(&self) -> Option<AwbcTrapCode> {
        match self {
            Self::NestedCallExit(exit) => Some(match exit.as_ref() {
                VmNestedCallExit::Trapped(trap) => trap.code,
                VmNestedCallExit::Suspended(_)
                | VmNestedCallExit::DialogueResultSelected(_)
                | VmNestedCallExit::Cancelled
                | VmNestedCallExit::BudgetYield(_) => AwbcTrapCode::InternalInvariant,
            }),
            Self::Evaluation(error) => Some(match error.recoverable_expression() {
                Some(crate::value::RuntimeExpressionFailure::DivisionByZero) => {
                    AwbcTrapCode::DivisionByZero
                }
                Some(crate::value::RuntimeExpressionFailure::OptionUnwrapNone) => {
                    AwbcTrapCode::ExplicitPanic
                }
                Some(crate::value::RuntimeExpressionFailure::IndexOutOfBounds { .. }) => {
                    AwbcTrapCode::InvalidIndex
                }
                None => AwbcTrapCode::InternalInvariant,
            }),
            Self::PatternMismatch => Some(AwbcTrapCode::PatternMismatch),
            Self::DynamicTarget(_) => Some(AwbcTrapCode::MissingDynamicTarget),
            Self::Runtime(_) => Some(AwbcTrapCode::InternalInvariant),
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
            | Self::MissingPureHelper(_)
            | Self::MissingTraitMethod(_)
            | Self::MissingLineOperation(_) => None,
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
