//! Flow-evaluated formatter attempt state and continuation handling.

mod state;
pub use state::{FiberFormatAttemptState, FiberFormatState};
pub(super) use state::{FormatAttempt, FormatOperandProgress, FormatStaging};

use super::{
    AwbcBlockId, AwbcFunctionId, AwbcInstruction, AwbcProgram, AwbcRegisterId, FiberCursor,
    FiberFrame, FiberState, FiberStateError, FiberStatus, RuntimeFormatContext, RuntimeValue,
    instruction_at_site, validate_format_state, validate_return_point,
};
use crate::awbc::schema::AwbcFormatAttemptOperand;
use crate::runtime_id::RuntimeFormatAttemptId;
use crate::value::RuntimeFmtParameterId;

impl FiberState {
    pub(crate) fn begin_format_operand_attempt(
        &mut self,
        program: &AwbcProgram,
        context: &RuntimeFormatContext,
        attempt: RuntimeFormatAttemptId,
        parameter: RuntimeFmtParameterId,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let site = format_attempt_site(program, attempt)?;
        let Some((site_attempt, manifest)) = format_attempt_operands_at_site(program, site)? else {
            return Err(FiberStateError::InvalidFrame);
        };
        if site_attempt != attempt || site.function != self.active_frame()?.function {
            return Err(FiberStateError::InvalidFrame);
        }

        let frame = self.active_frame_mut()?;
        let scope_depth = frame.scopes.len();
        let matching_index = frame
            .format_attempts
            .iter()
            .rposition(|state| state.storage.attempt == attempt);
        let state = match matching_index {
            Some(index) if index + 1 == frame.format_attempts.len() => {
                &mut frame.format_attempts[index]
            }
            Some(_) => return Err(FiberStateError::InvalidFrame),
            None => {
                if manifest.first().map(|operand| operand.parameter) != Some(parameter) {
                    return Err(FiberStateError::InvalidFrame);
                }
                let state = FiberFormatAttemptState {
                    storage: FormatAttempt {
                        attempt,
                        staging: FormatStaging::new(site, context, manifest.len())?,
                        progress: FormatOperandProgress::Evaluating {
                            parameter,
                            scope_depth,
                            failure: None,
                        },
                    },
                };
                frame.format_attempts.push(state);
                return Ok(());
            }
        };
        if state.storage.staging.site != site
            || !matches!(state.storage.progress, FormatOperandProgress::Idle)
            || state.storage.staging.next_operand >= manifest.len()
            || manifest
                .get(state.storage.staging.next_operand)
                .map(|operand| operand.parameter)
                != Some(parameter)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        state.storage.progress = FormatOperandProgress::Evaluating {
            parameter,
            scope_depth,
            failure: None,
        };
        Ok(())
    }

    pub(crate) fn complete_format_operand_attempt(
        &mut self,
        program: &AwbcProgram,
        attempt: RuntimeFormatAttemptId,
        parameter: RuntimeFmtParameterId,
        value_register: AwbcRegisterId,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let frame = self.active_frame()?;
        let state = frame
            .format_attempts
            .last()
            .filter(|state| state.storage.attempt == attempt)
            .ok_or(FiberStateError::InvalidFrame)?;
        let manifest = format_attempt_operands_at_site(program, state.storage.staging.site)?
            .filter(|(candidate, _)| *candidate == attempt)
            .map(|(_, manifest)| manifest)
            .ok_or(FiberStateError::InvalidFrame)?;
        let ordinal = state.storage.staging.next_operand;
        if state.active_parameter() != Some(parameter)
            || manifest.get(ordinal).map(|operand| operand.parameter) != Some(parameter)
            || state
                .storage
                .staging
                .values
                .get(ordinal)
                .is_none_or(Option::is_some)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let failed = matches!(
            state.storage.progress,
            FormatOperandProgress::Evaluating {
                failure: Some(_),
                ..
            }
        );
        let expected_type = manifest[ordinal].ty;
        if !failed {
            let value = frame
                .registers
                .get(value_register.index())
                .and_then(crate::value::RuntimePlaceStorage::as_ref)
                .ok_or(FiberStateError::RegisterOutOfBounds {
                    register: value_register.0,
                    layout: frame.layout.0,
                })?;
            crate::program_types::RuntimeProgramValueType::Awbc {
                program,
                ty: expected_type,
            }
            .validate_view(
                value.view(),
                frame.type_instantiation.as_deref(),
                &mut crate::entry::schema::value_encoding::ValueResourceAdmission::live(
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                ),
            )
            .map_err(|error| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                    Box::new(error.within_value_path(&format!(
                        "format_attempt[{}].values[{ordinal}]",
                        attempt.index()
                    ))),
                ))
            })?;
        }
        let frame = self.active_frame_mut()?;
        let value = if failed {
            frame.clear_register(value_register)?;
            None
        } else {
            Some(frame.take_register(value_register)?)
        };
        let state = frame
            .format_attempts
            .last_mut()
            .filter(|state| state.storage.attempt == attempt)
            .ok_or(FiberStateError::InvalidFrame)?;
        state.storage.staging.values[ordinal] = value;
        let FormatOperandProgress::Evaluating { failure, .. } =
            std::mem::replace(&mut state.storage.progress, FormatOperandProgress::Idle)
        else {
            unreachable!("the exact active parameter was checked before taking its value");
        };
        if failed && state.storage.staging.first_recoverable.is_none() {
            state.storage.staging.first_recoverable = failure;
        }
        state.storage.staging.next_operand = state
            .storage
            .staging
            .next_operand
            .checked_add(1)
            .ok_or(FiberStateError::InvalidFrame)?;
        Ok(())
    }

    pub(crate) fn abandon_format_operand_attempt(
        &mut self,
        attempt: RuntimeFormatAttemptId,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let frame = self.active_frame_mut()?;
        if frame
            .format_attempts
            .last()
            .map(|state| state.storage.attempt)
            != Some(attempt)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        frame.format_attempts.pop();
        Ok(())
    }

    pub(crate) fn adopt_completed_format_attempt(
        &mut self,
        program: &AwbcProgram,
        attempt: RuntimeFormatAttemptId,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let site = self.cursor;
        let Some((site_attempt, manifest)) = format_attempt_operands_at_site(program, site)? else {
            return Err(FiberStateError::InvalidFrame);
        };
        let frame = self.active_frame()?;
        if site_attempt != attempt || site.function != frame.function {
            return Err(FiberStateError::InvalidFrame);
        }
        if let Some(format) = &frame.format {
            if !frame.format_attempts.is_empty() || format.site() != site {
                return Err(FiberStateError::InvalidFrame);
            }
            return validate_format_state(program, frame, format);
        }
        let state = frame
            .format_attempts
            .last()
            .filter(|state| state.storage.attempt == attempt)
            .ok_or(FiberStateError::InvalidFrame)?;
        validate_format_attempt_state(program, frame, state)?;
        if state.storage.staging.site != site
            || state.storage.staging.next_operand != manifest.len()
            || !matches!(state.storage.progress, FormatOperandProgress::Idle)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let state = self
            .active_frame_mut()?
            .format_attempts
            .pop()
            .ok_or(FiberStateError::InvalidFrame)?;
        let frame = self.active_frame_mut()?;
        frame.format = Some(FiberFormatState {
            storage: state.storage.staging,
        });
        let format = frame.format.as_ref().ok_or(FiberStateError::InvalidFrame)?;
        validate_format_state(program, frame, format)
    }

    pub(super) fn recover_format_attempt_operand(
        &mut self,
        program: &AwbcProgram,
        reason: String,
    ) -> Result<bool, FiberStateError> {
        let Some((owner_index, attempt_index, site, parameter)) = self
            .frames
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, frame)| {
                frame.format_attempts.iter().enumerate().rev().find_map(
                    |(attempt_index, attempt)| {
                        attempt
                            .active_parameter()
                            .map(|parameter| (index, attempt_index, attempt.site(), parameter))
                    },
                )
            })
        else {
            return Ok(false);
        };
        let owner = self
            .frames
            .get(owner_index)
            .ok_or(FiberStateError::InvalidFrame)?;
        let state = owner
            .format_attempts
            .get(attempt_index)
            .ok_or(FiberStateError::InvalidFrame)?;
        validate_format_attempt_state(program, owner, state)?;
        let attempt = state.storage.attempt;
        let ordinal = state.storage.staging.next_operand;
        let FormatOperandProgress::Evaluating { scope_depth, .. } = state.storage.progress else {
            return Err(FiberStateError::InvalidFrame);
        };
        let completion = format_attempt_completion_cursor(
            program,
            owner.function,
            attempt,
            parameter,
            crate::entry::schema::value_budget::ValidationWork::new(
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
        )?;
        for child_index in owner_index + 1..self.frames.len() {
            let caller = self
                .frames
                .get(child_index - 1)
                .ok_or(FiberStateError::InvalidFrame)?;
            let child = self
                .frames
                .get(child_index)
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
        }
        if site != state.storage.staging.site
            || self.frames[owner_index + 1..]
                .iter()
                .any(FiberFrame::has_pending_cleanup)
            || owner.scopes[scope_depth.min(owner.scopes.len())..]
                .iter()
                .any(super::scope_has_pending_cleanup)
            || scope_depth > owner.scopes.len()
            || state
                .storage
                .staging
                .values
                .get(ordinal)
                .is_none_or(Option::is_some)
        {
            return Ok(false);
        }
        FormatStaging::<RuntimeValue>::validate_failure_message(&reason)?;
        self.frames.truncate(owner_index + 1);
        let owner = self
            .frames
            .get_mut(owner_index)
            .ok_or(FiberStateError::InvalidFrame)?;
        owner.scopes.truncate(scope_depth);
        let state = owner
            .format_attempts
            .get_mut(attempt_index)
            .ok_or(FiberStateError::InvalidFrame)?;
        if state.storage.staging.first_recoverable.is_none() {
            state.storage.staging.first_recoverable = Some(reason.clone());
        }
        let FormatOperandProgress::Evaluating { failure, .. } = &mut state.storage.progress else {
            return Err(FiberStateError::InvalidFrame);
        };
        *failure = Some(reason);
        self.cursor = completion;
        Ok(true)
    }

    pub(super) fn discard_format_attempts(&mut self) {
        for frame in &mut self.frames {
            frame.format_attempts.clear();
        }
    }
}

pub(super) fn format_attempt_operands_at_site(
    program: &AwbcProgram,
    site: FiberCursor,
) -> Result<Option<(RuntimeFormatAttemptId, &[AwbcFormatAttemptOperand])>, FiberStateError> {
    let AwbcInstruction::FormatContent {
        attempt: Some(attempt),
        attempt_operands,
        ..
    } = instruction_at_site(program, site)?
    else {
        return Ok(None);
    };
    Ok(Some((*attempt, attempt_operands)))
}

fn format_attempt_site(
    program: &AwbcProgram,
    attempt: RuntimeFormatAttemptId,
) -> Result<FiberCursor, FiberStateError> {
    let mut found = None;
    for (function_index, function) in program.functions.iter().enumerate() {
        let function_id = AwbcFunctionId(
            u32::try_from(function_index).map_err(|_| FiberStateError::InvalidFrame)?,
        );
        let Some(end) = function.blocks.checked_end() else {
            return Err(FiberStateError::InvalidFrame);
        };
        for block_index in function.blocks.start..end {
            let block_id = AwbcBlockId(block_index);
            let block = program
                .blocks
                .get(block_id.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            let Some(instruction_end) = block.instructions.checked_end() else {
                return Err(FiberStateError::InvalidFrame);
            };
            for instruction_index in block.instructions.start..instruction_end {
                if matches!(
                    program.instructions.get(instruction_index as usize),
                    Some(AwbcInstruction::FormatContent {
                        attempt: Some(candidate),
                        ..
                    }) if *candidate == attempt
                ) {
                    if found.is_some() {
                        return Err(FiberStateError::InvalidFrame);
                    }
                    found = Some(FiberCursor {
                        function: function_id,
                        block: block_id,
                        instruction_offset: instruction_index - block.instructions.start,
                    });
                }
            }
        }
    }
    found.ok_or(FiberStateError::InvalidFrame)
}

fn format_attempt_completion_cursor(
    program: &AwbcProgram,
    function_id: AwbcFunctionId,
    attempt: RuntimeFormatAttemptId,
    parameter: RuntimeFmtParameterId,
    mut work: crate::entry::schema::value_budget::ValidationWork,
) -> Result<FiberCursor, FiberStateError> {
    work.charge(0)
        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
    let function = program
        .functions
        .get(function_id.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let Some(end) = function.blocks.checked_end() else {
        return Err(FiberStateError::InvalidFrame);
    };
    let mut found = None;
    work.collection(function.blocks.len as usize)
        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
    for block_index in function.blocks.start..end {
        work.charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let block_id = AwbcBlockId(block_index);
        let block = program
            .blocks
            .get(block_id.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let Some(instruction_end) = block.instructions.checked_end() else {
            return Err(FiberStateError::InvalidFrame);
        };
        work.collection(block.instructions.len as usize)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        for instruction_index in block.instructions.start..instruction_end {
            work.charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if matches!(
                program.instructions.get(instruction_index as usize),
                Some(AwbcInstruction::CompleteFormatOperand {
                    attempt: candidate,
                    parameter: candidate_parameter,
                    ..
                }) if *candidate == attempt && *candidate_parameter == parameter
            ) {
                if found.is_some() {
                    return Err(FiberStateError::InvalidFrame);
                }
                found = Some(FiberCursor {
                    function: function_id,
                    block: block_id,
                    instruction_offset: instruction_index - block.instructions.start,
                });
            }
        }
    }
    found.ok_or(FiberStateError::InvalidFrame)
}

pub(super) fn validate_format_attempt_state(
    program: &AwbcProgram,
    frame: &FiberFrame,
    state: &FiberFormatAttemptState,
) -> Result<(), FiberStateError> {
    let context = super::FrameValueContext {
        program,
        function: frame.function,
        layout: program
            .frame_layouts
            .get(frame.layout.index())
            .ok_or(FiberStateError::InvalidFrame)?,
        binding: frame.type_instantiation.as_deref(),
        registers: &frame.registers,
        scope_count: frame.scopes.len(),
    };
    state.validate_inputs(
        &context,
        &mut crate::entry::schema::value_encoding::ValueResourceAdmission::live(
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ),
        RuntimeValue::view,
    )
}
