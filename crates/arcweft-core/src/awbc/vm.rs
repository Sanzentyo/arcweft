//! Compact AWBC VM executor.
//!
//! The VM is Sans I/O. Host operations, line tasks, dialogue, choice, await,
//! await-many and budget yields are surfaced as typed exits over `FiberState`.
//! This module never falls back to the structured VM.

use super::fiber::{
    FiberAwaitManyState, FiberSafePoint, FiberState, FiberStateError, FiberStatus, FiberSuspension,
    FiberSuspensionReason, FiberTerminalValue, FiberTrap,
};
use super::schema::{
    AwbcBinaryOp, AwbcBlockId, AwbcCodeLocation, AwbcConstant, AwbcConstantId, AwbcContentUnitId,
    AwbcEffectPlanId, AwbcFunctionId, AwbcInstruction, AwbcInstructionId, AwbcIntrinsicId,
    AwbcOpcode, AwbcPattern, AwbcPatternId, AwbcProgram, AwbcPureHelperId, AwbcRegisterId,
    AwbcResumePointId, AwbcSignedIntKind, AwbcSourceMapId, AwbcSourcePlanId, AwbcStreamPlanId,
    AwbcStringId, AwbcTaskPlanId, AwbcTerminator, AwbcTrapCode, AwbcTypeId, AwbcUnaryOp,
    AwbcUnsignedIntKind,
};
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeSeq, RuntimeValue, evaluate_binary, evaluate_unary,
    runtime_sequence_from_literal_values, runtime_sequence_repeat_value, runtime_value_label,
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
    SourceYield {
        source: AwbcSourcePlanId,
        value: RuntimeValue,
    },
    SourceClose(AwbcSourcePlanId),
    Trap(FiberTrap),
}

#[derive(Clone, Debug, PartialEq)]
pub enum VmExit {
    Running,
    Suspended(FiberSuspensionReason),
    Returned(Option<RuntimeValue>),
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
    #[error("runtime error: {0}")]
    Runtime(String),
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
                resume: safe_point.resume.unwrap_or(AwbcResumePointId(0)),
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
            if let Err(error) = execute_instruction(
                program,
                fiber,
                host,
                instruction,
                source_map,
                &mut observations,
            ) {
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
            fiber.cursor.instruction_offset = fiber.cursor.instruction_offset.saturating_add(1);
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
) -> Result<(), VmError> {
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
                .push(super::fiber::FiberScope { id: *scope, depth });
        }
        AwbcInstruction::ExitScope { .. } => {
            fiber.active_frame_mut()?.scopes.pop();
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
                    return Ok(());
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
        AwbcInstruction::MakeRecord { dst, fields, .. } => {
            let fields = fields
                .iter()
                .enumerate()
                .map(|(index, register_id)| {
                    Ok(crate::value::RuntimeFieldValue {
                        name: format!("field{index}"),
                        value: register(fiber, *register_id)?.clone(),
                    })
                })
                .collect::<Result<Vec<_>, VmError>>()?;
            fiber
                .active_frame_mut()?
                .set_register(*dst, RuntimeValue::Record(fields))?;
        }
        AwbcInstruction::MakeVariant {
            dst, case, payload, ..
        } => {
            let payload = payload
                .map(|payload| register(fiber, payload).cloned())
                .transpose()?
                .map(Box::new);
            fiber.active_frame_mut()?.set_register(
                *dst,
                RuntimeValue::Variant {
                    path: None,
                    name: format!("case{case}"),
                    payload,
                },
            )?;
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
                .map(|field| field.value.clone())
                .ok_or_else(|| VmError::Runtime("record projection out of bounds".to_owned()))?;
            fiber.active_frame_mut()?.set_register(*dst, value)?;
        }
        AwbcInstruction::ProjectField { dst, target, field } => {
            let field = string(program, *field)?;
            let RuntimeValue::Record(items) = register(fiber, *target)? else {
                return Err(VmError::Runtime(
                    "field projection expected record".to_owned(),
                ));
            };
            let value = items
                .iter()
                .find(|item| item.name == field)
                .map(|field| field.value.clone())
                .ok_or_else(|| VmError::Runtime(format!("missing field `{field}`")))?;
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
        AwbcInstruction::SourceClose { source } => {
            if let Some(state) = fiber.sources.iter_mut().find(|state| state.plan == *source)
                && !state.closed
            {
                state.closed = true;
                observations.push(VmObservation::SourceClose(*source));
            }
        }
        AwbcInstruction::SourceYield { source, value } => {
            let value = register(fiber, *value)?.clone();
            observations.push(VmObservation::SourceYield {
                source: *source,
                value: value.clone(),
            });
            if let Some(state) = fiber.sources.iter_mut().find(|state| state.plan == *source)
                && !state.closed
            {
                state.queue.push(value);
            }
        }
    }
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
            let condition = register(fiber, *condition)?
                .as_bool()
                .ok_or_else(|| VmError::Runtime("branch condition expected bool".to_owned()))?;
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
            fiber.replace_active_function(program, *function, &args)?;
            Ok(VmExit::Running)
        }
        AwbcTerminator::GotoDynamic { target, args } => {
            let target_value = register(fiber, *target)?.clone();
            let target = match &target_value {
                RuntimeValue::String(target) | RuntimeValue::EntityRef(target) => program
                    .functions
                    .iter()
                    .enumerate()
                    .find_map(|(index, function)| {
                        function
                            .public_id
                            .and_then(|id| program.strings.get(id.index()))
                            .filter(|public_id| *public_id == target)
                            .and_then(|_| u32::try_from(index).ok())
                            .map(AwbcFunctionId)
                    }),
                _ => None,
            }
            .ok_or_else(|| {
                VmError::Runtime(format!(
                    "missing dynamic goto target `{}`",
                    runtime_value_label(&target_value)
                ))
            })?;
            let args = register_values(fiber, args)?;
            fiber.replace_active_function(program, target, &args)?;
            Ok(VmExit::Running)
        }
        AwbcTerminator::Dialogue {
            content,
            line_task_group,
            resume,
        } => suspend(
            fiber,
            *resume,
            FiberSuspensionReason::Dialogue {
                content: *content,
                line_task_group: *line_task_group,
            },
        ),
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
            task,
            binding,
            resume,
        } => suspend(
            fiber,
            *resume,
            FiberSuspensionReason::Await {
                task: register(fiber, *task)?.clone(),
                binding: *binding,
            },
        ),
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
            let trap = FiberTrap {
                code: *code,
                message,
                source_map,
            };
            observations.push(VmObservation::Trap(trap.clone()));
            fiber.mark_trapped(trap.clone());
            Ok(VmExit::Trapped(trap))
        }
        AwbcTerminator::BudgetYield { resume } => {
            suspend(fiber, *resume, FiberSuspensionReason::BudgetYield)
        }
        AwbcTerminator::Unreachable => {
            let trap = FiberTrap {
                code: AwbcTrapCode::InternalInvariant,
                message: Some("unreachable AWBC block executed".to_owned()),
                source_map,
            };
            observations.push(VmObservation::Trap(trap.clone()));
            fiber.mark_trapped(trap.clone());
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
        resume,
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
        AwbcConstant::Record { fields, .. } => Ok(RuntimeValue::Record(
            fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    Ok(crate::value::RuntimeFieldValue {
                        name: format!("field{index}"),
                        value: constant_value(program, *field)?,
                    })
                })
                .collect::<Result<Vec<_>, VmError>>()?,
        )),
        AwbcConstant::Variant { case, payload, .. } => Ok(RuntimeValue::Variant {
            path: None,
            name: format!("case{case}"),
            payload: payload
                .map(|id| constant_value(program, id))
                .transpose()?
                .map(Box::new),
        }),
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
        AwbcPattern::Bind { .. } | AwbcPattern::Discard => true,
        AwbcPattern::Literal(id) => constant_value(program, *id)? == *value,
        AwbcPattern::Entity(id) => {
            matches!(value, RuntimeValue::EntityRef(actual) if actual == string(program, *id)?)
        }
        AwbcPattern::Tuple(patterns) => {
            matches!(value, RuntimeValue::Tuple(values) if values.len() == patterns.len() && patterns.iter().zip(values).all(|(pattern, value)| test_pattern(program, *pattern, value).unwrap_or(false)))
        }
        AwbcPattern::Record { fields, .. } => {
            matches!(value, RuntimeValue::Record(values) if fields.iter().all(|field| values.get(field.field as usize).is_some_and(|value| test_pattern(program, field.pattern, &value.value).unwrap_or(false))))
        }
        AwbcPattern::Sequence { items, .. } => {
            matches!(value, RuntimeValue::Seq(sequence) if sequence.len() >= items.len() && items.iter().enumerate().all(|(index, pattern)| test_pattern(program, *pattern, &sequence.value_at(index)).unwrap_or(false)))
        }
        AwbcPattern::Variant { case, payload, .. } => {
            matches!(value, RuntimeValue::Variant { name, payload: actual, .. } if format!("case{case}") == *name && payload.is_none_or(|pattern| actual.as_deref().is_some_and(|value| test_pattern(program, pattern, value).unwrap_or(false))))
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
    let pattern_record = program
        .patterns
        .get(pattern.index())
        .ok_or(VmError::MissingPattern(pattern))?
        .clone();
    match pattern_record {
        AwbcPattern::Bind { target, .. } | AwbcPattern::Whole { target, .. } => {
            fiber
                .active_frame_mut()?
                .set_register(target, value.clone())?;
        }
        AwbcPattern::Tuple(children) => {
            if let RuntimeValue::Tuple(values) = value {
                for (child, value) in children.into_iter().zip(values) {
                    bind_pattern(program, fiber, child, value)?;
                }
            }
        }
        AwbcPattern::Sequence { items, rest } => {
            if let RuntimeValue::Seq(sequence) = value {
                let item_count = items.len();
                for (index, child) in items.iter().copied().enumerate() {
                    bind_pattern(program, fiber, child, &sequence.value_at(index))?;
                }
                if let Some(rest) = rest {
                    fiber
                        .active_frame_mut()?
                        .set_register(rest, RuntimeValue::Seq(sequence.tail_from(item_count)))?;
                }
            }
        }
        _ => {
            if !test_pattern(program, pattern, value)? {
                return Err(VmError::Runtime("pattern did not match".to_owned()));
            }
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
    let trap = FiberTrap {
        code,
        message: message.map(str::to_owned),
        source_map,
    };
    observations.push(VmObservation::Trap(trap.clone()));
    fiber.mark_trapped(trap);
}

fn mark_runtime_error_trap(
    fiber: &mut FiberState,
    code: AwbcTrapCode,
    message: String,
    source_map: Option<AwbcSourceMapId>,
    observations: &mut Vec<VmObservation>,
) -> FiberTrap {
    let trap = FiberTrap {
        code,
        message: Some(message),
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
            ) => Some(AwbcTrapCode::TypeMismatch),
            Self::MissingIntrinsic(_) => Some(AwbcTrapCode::HostAbiMismatch),
            Self::Fiber(_) => Some(AwbcTrapCode::InternalInvariant),
            Self::MissingFunction(_)
            | Self::MissingBlock(_)
            | Self::MissingInstruction(_)
            | Self::MissingConstant(_)
            | Self::MissingString(_)
            | Self::MissingPattern(_)
            | Self::MissingType(_) => None,
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
