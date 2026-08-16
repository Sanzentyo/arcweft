//! Compact AWBC VM executor.
//!
//! The VM is Sans I/O. Host operations, line tasks, dialogue, choice, await,
//! await-many and budget yields are surfaced as typed exits over `FiberState`.
//! This module never falls back to the structured VM.

use super::fiber::{
    FiberAwaitManyState, FiberAwaitTarget, FiberCursor, FiberResumeTarget, FiberReturnPoint,
    FiberSafePoint, FiberScopeCleanup, FiberState, FiberStateError, FiberStatus, FiberSuspension,
    FiberSuspensionReason, FiberTerminalValue, FiberTrap, runtime_value_matches_type,
    runtime_variant_identity,
};
use super::schema::{
    AwbcBinaryOp, AwbcBlockId, AwbcCodeLocation, AwbcConstant, AwbcConstantId, AwbcContentUnitId,
    AwbcEffectPlanId, AwbcFunctionId, AwbcInstruction, AwbcInstructionId, AwbcIntrinsicId,
    AwbcOpcode, AwbcPattern, AwbcPatternId, AwbcPatternRest, AwbcProgram, AwbcPureHelperId,
    AwbcRegisterId, AwbcResumePointId, AwbcRuntimeType, AwbcSignedIntKind, AwbcSourceMapId,
    AwbcStreamPlanId, AwbcStringId, AwbcTaskPlanId, AwbcTerminator, AwbcTraitMethodId,
    AwbcTraitReceiverMode, AwbcTrapCode, AwbcTypeId, AwbcUnaryOp, AwbcUnsignedIntKind,
};
use crate::task::NeedId;
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeAgentValue, RuntimeBinding, RuntimeFunctionBody, RuntimeFunctionValue,
    RuntimeNominalRecordValue, RuntimeReductionValue, RuntimeSeq, RuntimeValue, evaluate_binary,
    evaluate_unary, runtime_sequence_from_literal_values, runtime_sequence_repeat_value,
    runtime_value_label,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmStepOptions {
    pub max_instructions: u64,
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
            if control == InstructionControl::Continue {
                if fiber.cursor != cursor {
                    return Err(VmError::Runtime(
                        "instruction changed control flow without reporting a transfer".to_owned(),
                    ));
                }
                fiber.cursor.instruction_offset = cursor.instruction_offset.saturating_add(1);
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
            let value = register(fiber, *src)?.clone();
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::Clear { register } | AwbcInstruction::Drop { register } => {
            fiber.active_frame_mut()?.clear_register(*register)?;
        }
        AwbcInstruction::EnterScope { scope } => {
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
        AwbcInstruction::ExitScope { .. } => {
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
        AwbcInstruction::MakeRecord {
            dst,
            ty,
            field_names,
            fields,
        } => {
            let value = match program.runtime_types.get(ty.index()) {
                Some(AwbcRuntimeType::NominalRecord { .. }) => {
                    let layout = program
                        .nominal_record_layout(*ty)
                        .map_err(|error| VmError::Runtime(error.to_string()))?
                        .expect("nominal-record AWBC row projects a nominal-record layout");
                    let fields = fields
                        .iter()
                        .map(|register_id| register(fiber, *register_id).cloned())
                        .collect::<Result<Vec<_>, _>>()?;
                    RuntimeNominalRecordValue::try_from_accepted_layout(&layout, fields)
                        .map(RuntimeValue::NominalRecord)
                        .map_err(|error| VmError::Runtime(error.to_string()))?
                }
                Some(AwbcRuntimeType::Record { .. } | AwbcRuntimeType::Dynamic) => {
                    let fields = fields
                        .iter()
                        .zip(field_names)
                        .map(|(register_id, field_name)| {
                            Ok((
                                string(program, *field_name)?.to_owned(),
                                register(fiber, *register_id)?.clone(),
                            ))
                        })
                        .collect::<Result<Vec<_>, VmError>>()?;
                    RuntimeValue::try_record(fields)
                        .map_err(|error| VmError::Runtime(error.to_string()))?
                }
                Some(_) => {
                    return Err(VmError::Runtime(
                        "record construction references a non-record type".to_owned(),
                    ));
                }
                None => return Err(VmError::MissingType(*ty)),
            };
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
            let RuntimeValue::Record(items) = register(fiber, *target)? else {
                return Err(VmError::Runtime(
                    "record projection expected record".to_owned(),
                ));
            };
            let value = items
                .get(*ordinal as usize)
                .map(|field| field.value().clone())
                .ok_or_else(|| VmError::Runtime("record projection out of bounds".to_owned()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::ProjectField { dst, target, field } => {
            let field = string(program, *field)?;
            let value = match register(fiber, *target)? {
                RuntimeValue::Record(items) => items
                    .iter()
                    .find(|item| item.name() == field)
                    .map(|field| field.value().clone()),
                RuntimeValue::Agent(value) => value.project_field_label(field),
                _ => None,
            };
            let value =
                value.ok_or_else(|| VmError::Runtime(format!("missing field `{field}`")))?;
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
                execute_trait_method_call(program, fiber, host, *method, *receiver, args)?;
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
        AwbcInstruction::MakeFunction {
            dst,
            function,
            params,
            capture_names,
            captures,
        } => {
            let params = params
                .iter()
                .map(|param| string(program, *param).map(str::to_owned))
                .collect::<Result<Vec<_>, _>>()?;
            let captures = capture_names
                .iter()
                .zip(captures)
                .map(|(name, value)| {
                    Ok(RuntimeBinding {
                        name: string(program, *name)?.to_owned(),
                        value: register(fiber, *value)?.clone(),
                    })
                })
                .collect::<Result<Vec<_>, VmError>>()?;
            let value =
                RuntimeValue::Function(RuntimeFunctionValue::new_awbc(params, *function, captures));
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::ApplyFunction { dst, callee, args } => {
            let callee = register(fiber, *callee)?.clone();
            let args = register_values(fiber, args)?;
            let RuntimeValue::Function(function) = callee else {
                return Err(VmError::Runtime(format!(
                    "function application expected function, found {}",
                    runtime_value_label(&callee)
                )));
            };
            return apply_runtime_function(program, fiber, &function, &args, *dst);
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

fn apply_runtime_function(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    function: &RuntimeFunctionValue,
    args: &[RuntimeValue],
    destination: AwbcRegisterId,
) -> Result<InstructionControl, VmError> {
    let arity = function
        .remaining_arity()
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    if args.len() < arity {
        fiber.active_frame_mut()?.set_register(
            destination,
            RuntimeValue::Function(
                function
                    .try_bind_prefix(args)
                    .map_err(|error| VmError::Runtime(error.to_string()))?,
            ),
        )?;
        return Ok(InstructionControl::Continue);
    }
    if args.len() > arity {
        return Err(VmError::FunctionArgumentCount {
            expected: arity,
            actual: args.len(),
        });
    }
    let RuntimeFunctionBody::Awbc(closure) = function.body() else {
        return Err(VmError::Runtime(
            "AWBC VM cannot apply structured expression function bodies".to_owned(),
        ));
    };
    let mut values = closure
        .captures()
        .iter()
        .map(|capture| capture.value.clone())
        .collect::<Vec<_>>();
    values.extend(args.iter().cloned());
    let caller = fiber.cursor;
    let return_to = FiberReturnPoint {
        cursor: FiberCursor {
            function: caller.function,
            block: caller.block,
            instruction_offset: caller.instruction_offset.saturating_add(1),
        },
        destination: Some(destination),
    };
    fiber.push_call_frame_at(program, closure.function(), return_to, &values)?;
    Ok(InstructionControl::Transferred)
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
        let output = step_with_host(
            program,
            &mut method_fiber,
            VmStepOptions {
                max_instructions: TRAIT_METHOD_BUDGET,
            },
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
    let RuntimeValue::Record(fields) = target else {
        return Err(VmError::Runtime(format!(
            "field assignment expected record, found {}",
            runtime_value_label(target)
        )));
    };
    let Some(field_value) = fields.get_mut(field as usize) else {
        return Err(VmError::Runtime(format!(
            "missing record field ordinal {field}"
        )));
    };
    *field_value.value_mut() = value;
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "AWBC terminator dispatch keeps the shared fiber/suspension state machine in one match"
)]
fn execute_terminator(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    _host: &mut impl VmHost,
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
        AwbcTerminator::GotoStatic { function, args } => {
            let args = register_values(fiber, args)?;
            drain_active_frame_cleanups(fiber, observations)?;
            fiber.replace_active_function(program, *function, &args)?;
            observations.push(VmObservation::Goto(*function));
            Ok(VmExit::Running)
        }
        AwbcTerminator::GotoDynamic { target, args } => {
            let target_value = register(fiber, *target)?.clone();
            let target = match &target_value {
                RuntimeValue::String(target) | RuntimeValue::EntityRef(target) => program
                    .resolve_flow_target_value(target)
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
            drain_active_frame_cleanups(fiber, observations)?;
            fiber.replace_active_function(program, target, &args)?;
            observations.push(VmObservation::Goto(target));
            Ok(VmExit::Running)
        }
        AwbcTerminator::Dialogue {
            content,
            values,
            line_task_captures,
            resume,
        } => {
            let values = values
                .iter()
                .map(|binding| {
                    Ok(crate::plan::RuntimeDialogueValueBinding {
                        slot: binding.slot,
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
                    line_task_captures,
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
            resume,
        } => {
            let target = await_target(program, fiber, *handle)?;
            suspend(
                fiber,
                *resume,
                FiberSuspensionReason::Await {
                    target,
                    binding: *binding,
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
            if fiber.finish_return(program, value.clone())? {
                Ok(VmExit::Returned(value))
            } else {
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

fn register(fiber: &FiberState, register: AwbcRegisterId) -> Result<&RuntimeValue, VmError> {
    fiber
        .active_frame()?
        .register(register)
        .map_err(VmError::from)
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
    match runtime_type {
        AwbcRuntimeType::NeedHandle => match value {
            RuntimeValue::String(need) if !need.is_empty() => {
                Ok(FiberAwaitTarget::Need(NeedId(need)))
            }
            value => Err(VmError::Runtime(format!(
                "NeedHandle register contained {}",
                runtime_value_label(&value)
            ))),
        },
        AwbcRuntimeType::TaskHandle | AwbcRuntimeType::Dynamic => Ok(FiberAwaitTarget::Task(value)),
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
        AwbcConstant::EntityRef(id) => {
            Ok(RuntimeValue::EntityRef(string(program, *id)?.to_owned()))
        }
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
        AwbcConstant::Record {
            ty,
            field_names,
            fields,
        } => {
            let values = fields
                .iter()
                .map(|field| constant_value(program, *field))
                .collect::<Result<Vec<_>, VmError>>()?;
            match program.runtime_types.get(ty.index()) {
                Some(AwbcRuntimeType::NominalRecord { .. }) => {
                    let layout = program
                        .nominal_record_layout(*ty)
                        .map_err(|error| VmError::Runtime(error.to_string()))?
                        .expect("nominal-record AWBC row projects a nominal-record layout");
                    RuntimeNominalRecordValue::try_from_accepted_layout(&layout, values)
                        .map(RuntimeValue::NominalRecord)
                        .map_err(|error| VmError::Runtime(error.to_string()))
                }
                Some(AwbcRuntimeType::Record { .. } | AwbcRuntimeType::Dynamic) => {
                    let fields = values
                        .into_iter()
                        .zip(field_names)
                        .map(|(value, field_name)| {
                            Ok((string(program, *field_name)?.to_owned(), value))
                        })
                        .collect::<Result<Vec<_>, VmError>>()?;
                    RuntimeValue::try_record(fields)
                        .map_err(|error| VmError::Runtime(error.to_string()))
                }
                Some(_) => Err(VmError::Runtime(
                    "record constant references a non-record type".to_owned(),
                )),
                None => Err(VmError::MissingType(*ty)),
            }
        }
        AwbcConstant::Variant {
            ty,
            case,
            case_name,
            payload,
        } => Ok(RuntimeValue::Variant {
            owner: variant_identity_for_type(program, *ty)?,
            ordinal: *case,
            name: string(program, *case_name)?.to_owned(),
            payload: payload
                .map(|id| constant_value(program, id))
                .transpose()?
                .map(Box::new),
        }),
        AwbcConstant::Opaque { ty, payload } => {
            let owner = program
                .opaque_owner(*ty)
                .map_err(|error| VmError::Runtime(error.to_string()))?
                .ok_or_else(|| {
                    VmError::Runtime("opaque constant references a non-opaque type".to_owned())
                })?;
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
        Some(AwbcRuntimeType::Variant { owner, .. }) => runtime_variant_identity(program, owner)
            .ok_or_else(|| VmError::Runtime("variant owner identity is invalid".to_owned())),
        Some(_) => Err(VmError::Runtime(
            "variant value references a non-variant runtime type".to_owned(),
        )),
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
        AwbcPattern::Entity(id) => {
            matches!(value, RuntimeValue::EntityRef(actual) if actual == string(program, *id)?)
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
                FiberStateError::ReturnValueMismatch | FiberStateError::EntryArgumentType { .. },
            )
            | Self::FunctionArgumentCount { .. } => Some(AwbcTrapCode::TypeMismatch),
            Self::MissingIntrinsic(_) => Some(AwbcTrapCode::HostAbiMismatch),
            Self::Fiber(_) => Some(AwbcTrapCode::InternalInvariant),
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
