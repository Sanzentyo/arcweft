//! Flow-evaluated formatter attempt state and continuation handling.

use super::{
    AwbcBlockId, AwbcFunctionId, AwbcInstruction, AwbcProgram, AwbcRegisterId, AwbcSaveResult,
    FiberCursor, FiberFormatState, FiberFrame, FiberState, FiberStateError, FiberStatus,
    RuntimeFormatContext, RuntimeProgramOwner, RuntimeValue, instruction_at_site,
    validate_format_state, validate_return_point, validate_runtime_value_at,
};
use crate::awbc::schema::AwbcFormatAttemptOperand;
use crate::runtime_id::RuntimeFormatAttemptId;
use crate::value::AwbcRuntimeValueSnapshot;
use crate::value::RuntimeFmtParameterId;
use serde::{Deserialize, Serialize};

mod optional_runtime_fmt_parameter_id {
    use crate::value::RuntimeFmtParameterId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(
        value: &Option<RuntimeFmtParameterId>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = value
            .map(|parameter| u8::try_from(parameter.index()).map_err(serde::ser::Error::custom))
            .transpose()?;
        encoded.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<RuntimeFmtParameterId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<u8>::deserialize(deserializer)?
            .map(|encoded| {
                RuntimeFmtParameterId::from_index(usize::from(encoded)).ok_or_else(|| {
                    serde::de::Error::custom(format_args!("unknown fmt parameter {encoded}"))
                })
            })
            .transpose()
    }
}

/// Typed in-progress transaction for Flow-evaluated formatter operands.
///
/// This stays on the owning frame while nested ProjectCall frames execute.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberFormatAttemptState {
    attempt: RuntimeFormatAttemptId,
    site: FiberCursor,
    format_context: RuntimeFormatContext,
    next_operand: usize,
    values: Vec<Option<RuntimeValue>>,
    first_recoverable: Option<String>,
    #[serde(with = "optional_runtime_fmt_parameter_id")]
    active_parameter: Option<RuntimeFmtParameterId>,
    active_failure: Option<String>,
    scope_depth: usize,
}

impl FiberFormatAttemptState {
    #[must_use]
    pub const fn attempt(&self) -> RuntimeFormatAttemptId {
        self.attempt
    }

    #[must_use]
    pub const fn site(&self) -> FiberCursor {
        self.site
    }

    #[must_use]
    pub const fn next_operand(&self) -> usize {
        self.next_operand
    }

    #[must_use]
    pub const fn active_parameter(&self) -> Option<RuntimeFmtParameterId> {
        self.active_parameter
    }

    pub fn values(&self) -> &[Option<RuntimeValue>] {
        &self.values
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcFiberFormatAttemptStateSnapshot {
    pub attempt: RuntimeFormatAttemptId,
    pub site: FiberCursor,
    pub format_context: RuntimeFormatContext,
    pub next_operand: usize,
    pub values: Vec<Option<AwbcRuntimeValueSnapshot>>,
    pub first_recoverable: Option<String>,
    #[serde(with = "optional_runtime_fmt_parameter_id")]
    pub active_parameter: Option<RuntimeFmtParameterId>,
    pub active_failure: Option<String>,
    pub scope_depth: usize,
}

impl AwbcFiberFormatAttemptStateSnapshot {
    pub(super) fn from_live(state: &FiberFormatAttemptState) -> AwbcSaveResult<Self> {
        Ok(Self {
            attempt: state.attempt,
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
            active_parameter: state.active_parameter,
            active_failure: state.active_failure.clone(),
            scope_depth: state.scope_depth,
        })
    }

    pub(super) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberFormatAttemptState> {
        Ok(FiberFormatAttemptState {
            attempt: self.attempt,
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
            active_parameter: self.active_parameter,
            active_failure: self.active_failure,
            scope_depth: self.scope_depth,
        })
    }
}

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
            .rposition(|state| state.attempt == attempt);
        let state = match matching_index {
            Some(index) if index + 1 == frame.format_attempts.len() => {
                &mut frame.format_attempts[index]
            }
            Some(_) => return Err(FiberStateError::InvalidFrame),
            None => {
                if manifest.first().map(|operand| operand.parameter) != Some(parameter) {
                    return Err(FiberStateError::InvalidFrame);
                }
                let mut state = FiberFormatAttemptState {
                    attempt,
                    site,
                    format_context: context.clone(),
                    next_operand: 0,
                    values: vec![None; manifest.len()],
                    first_recoverable: None,
                    active_parameter: None,
                    active_failure: None,
                    scope_depth,
                };
                state.active_parameter = Some(parameter);
                frame.format_attempts.push(state);
                return Ok(());
            }
        };
        if state.site != site
            || state.active_parameter.is_some()
            || state.next_operand >= manifest.len()
            || manifest
                .get(state.next_operand)
                .map(|operand| operand.parameter)
                != Some(parameter)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        state.active_parameter = Some(parameter);
        state.active_failure = None;
        state.scope_depth = scope_depth;
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
            .filter(|state| state.attempt == attempt)
            .ok_or(FiberStateError::InvalidFrame)?;
        let manifest = format_attempt_operands_at_site(program, state.site)?
            .filter(|(candidate, _)| *candidate == attempt)
            .map(|(_, manifest)| manifest)
            .ok_or(FiberStateError::InvalidFrame)?;
        let ordinal = state.next_operand;
        if state.active_parameter != Some(parameter)
            || manifest.get(ordinal).map(|operand| operand.parameter) != Some(parameter)
            || state.values.get(ordinal).is_none_or(Option::is_some)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let failed = state.active_failure.is_some();
        let expected_type = manifest[ordinal].ty;
        if !failed {
            let value = frame
                .registers
                .get(value_register.index())
                .and_then(Option::as_ref)
                .ok_or(FiberStateError::RegisterOutOfBounds {
                    register: value_register.0,
                    layout: frame.layout.0,
                })?;
            validate_runtime_value_at(
                program,
                value,
                Some(expected_type),
                format!("format_attempt[{}].values[{ordinal}]", attempt.index()),
            )?;
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
            .filter(|state| state.attempt == attempt)
            .ok_or(FiberStateError::InvalidFrame)?;
        state.values[ordinal] = value;
        if failed && state.first_recoverable.is_none() {
            state.first_recoverable = state.active_failure.clone();
        }
        state.next_operand = state
            .next_operand
            .checked_add(1)
            .ok_or(FiberStateError::InvalidFrame)?;
        state.active_parameter = None;
        state.active_failure = None;
        Ok(())
    }

    pub(crate) fn abandon_format_operand_attempt(
        &mut self,
        attempt: RuntimeFormatAttemptId,
    ) -> Result<(), FiberStateError> {
        self.require_status(FiberStatus::Running)?;
        let frame = self.active_frame_mut()?;
        if frame.format_attempts.last().map(|state| state.attempt) != Some(attempt) {
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
            if !frame.format_attempts.is_empty() || format.site != site {
                return Err(FiberStateError::InvalidFrame);
            }
            return validate_format_state(program, frame, format);
        }
        let state = frame
            .format_attempts
            .last()
            .filter(|state| state.attempt == attempt)
            .ok_or(FiberStateError::InvalidFrame)?;
        validate_format_attempt_state(program, frame, state)?;
        if state.site != site
            || state.next_operand != manifest.len()
            || state.active_parameter.is_some()
            || state.active_failure.is_some()
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
            site,
            format_context: state.format_context,
            next_operand: state.next_operand,
            values: state.values,
            first_recoverable: state.first_recoverable,
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
                            .active_parameter
                            .map(|parameter| (index, attempt_index, attempt.site, parameter))
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
        let attempt = state.attempt;
        let ordinal = state.next_operand;
        let scope_depth = state.scope_depth;
        let completion =
            format_attempt_completion_cursor(program, owner.function, attempt, parameter)?;
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
        if site != state.site
            || self.frames[owner_index + 1..]
                .iter()
                .any(FiberFrame::has_pending_cleanup)
            || owner.scopes[scope_depth.min(owner.scopes.len())..]
                .iter()
                .any(super::scope_has_pending_cleanup)
            || scope_depth > owner.scopes.len()
            || state.values.get(ordinal).is_none_or(Option::is_some)
        {
            return Ok(false);
        }
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
        if state.first_recoverable.is_none() {
            state.first_recoverable = Some(reason.clone());
        }
        state.active_failure = Some(reason);
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
) -> Result<FiberCursor, FiberStateError> {
    let function = program
        .functions
        .get(function_id.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    let Some(end) = function.blocks.checked_end() else {
        return Err(FiberStateError::InvalidFrame);
    };
    let mut found = None;
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
    if !state.format_context.has_current_data() || state.site.function != frame.function {
        return Err(FiberStateError::InvalidFrame);
    }
    let Some((attempt, manifest)) = format_attempt_operands_at_site(program, state.site)? else {
        return Err(FiberStateError::InvalidFrame);
    };
    if attempt != state.attempt
        || manifest.is_empty()
        || state.values.len() != manifest.len()
        || state.next_operand > manifest.len()
    {
        return Err(FiberStateError::InvalidFrame);
    }
    let mut had_failure = false;
    for (ordinal, (operand, value)) in manifest.iter().zip(&state.values).enumerate() {
        if ordinal < state.next_operand {
            if let Some(value) = value {
                validate_runtime_value_at(
                    program,
                    value,
                    Some(operand.ty),
                    format!(
                        "format_attempt[{}].values[{ordinal}]",
                        state.attempt.index()
                    ),
                )?;
            } else {
                had_failure = true;
            }
        } else if value.is_some() {
            return Err(FiberStateError::InvalidFrame);
        }
    }
    if (had_failure || state.active_failure.is_some()) != state.first_recoverable.is_some()
        || (state.active_failure.is_some() && state.active_parameter.is_none())
        || (state.active_failure.is_some() && state.first_recoverable.is_none())
    {
        return Err(FiberStateError::InvalidFrame);
    }
    if let Some(parameter) = state.active_parameter {
        if state.next_operand >= manifest.len()
            || manifest[state.next_operand].parameter != parameter
            || state.scope_depth > frame.scopes.len()
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let _ =
            format_attempt_completion_cursor(program, frame.function, state.attempt, parameter)?;
    }
    Ok(())
}
