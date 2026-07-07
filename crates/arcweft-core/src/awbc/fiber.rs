//! Executor-neutral AWBC fiber and safe-point state.

use super::schema::{
    AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcEffectPlanId, AwbcEntryId, AwbcEntryTarget,
    AwbcFrameLayoutId, AwbcFrameSlotRole, AwbcFunctionId, AwbcHostCallId, AwbcLineTaskGroupId,
    AwbcPatternId, AwbcProgram, AwbcRegisterId, AwbcResumePointId, AwbcRuntimeType, AwbcScopeId,
    AwbcSignatureId, AwbcSignedIntKind, AwbcSourceMapId, AwbcSourcePlanId, AwbcStreamPlanId,
    AwbcTaskPlanId, AwbcTrapCode, AwbcTypeId, AwbcUnsignedIntKind,
};
use crate::value::{RuntimeBinding, RuntimeInt, RuntimeUInt, RuntimeValue};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Complete state that may cross compact-VM and compiled-region boundaries.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberState {
    pub generation: u64,
    pub entry: AwbcEntryId,
    pub cursor: FiberCursor,
    pub frames: Vec<FiberFrame>,
    pub status: FiberStatus,
    pub suspension: Option<FiberSuspension>,
    pub terminal: Option<FiberTerminalValue>,
    pub budget: FiberBudget,
    pub line_cursor: u64,
    pub sources: Vec<FiberSourceState>,
    pub streams: Vec<FiberStreamState>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberCursor {
    pub function: AwbcFunctionId,
    pub block: AwbcBlockId,
    /// Offset in the block instruction range. A safe-point cursor is always zero.
    pub instruction_offset: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberFrame {
    pub function: AwbcFunctionId,
    pub layout: AwbcFrameLayoutId,
    pub return_to: Option<FiberReturnPoint>,
    pub registers: Vec<Option<RuntimeValue>>,
    pub root_cleanups: Vec<FiberScopeCleanup>,
    pub scopes: Vec<FiberScope>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberReturnPoint {
    pub resume: AwbcResumePointId,
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
    Trapped,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FiberSuspension {
    pub resume: AwbcResumePointId,
    pub reason: FiberSuspensionReason,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum FiberSuspensionReason {
    Dialogue {
        content: AwbcContentUnitId,
        line_task_group: AwbcLineTaskGroupId,
    },
    Choice {
        choice: AwbcChoiceId,
        destination: AwbcRegisterId,
    },
    Await {
        task: RuntimeValue,
        binding: Option<AwbcPatternId>,
    },
    AwaitMany(FiberAwaitManyState),
    HostCall {
        call: AwbcHostCallId,
        args: Vec<RuntimeValue>,
        destination: Option<AwbcRegisterId>,
    },
    BudgetYield,
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
pub struct FiberSourceState {
    pub plan: AwbcSourcePlanId,
    pub queue: Vec<RuntimeValue>,
    pub closed: bool,
    pub last_error: Option<RuntimeValue>,
    pub overflow_count: u64,
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
    Trapped(FiberTrap),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FiberStateError {
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
    #[error("fiber register {register} does not exist in frame layout {layout}")]
    RegisterOutOfBounds { register: u32, layout: u32 },
    #[error("fiber frame function/layout pair is invalid")]
    InvalidFrame,
    #[error("fiber call return value does not match its destination")]
    ReturnValueMismatch,
    #[error("AWBC entry expects {expected} arguments, received {actual}")]
    EntryArgumentCount { expected: usize, actual: usize },
    #[error("AWBC entry argument `{name}` is duplicated")]
    DuplicateEntryArgument { name: String },
    #[error("AWBC entry argument `{name}` does not match a parameter")]
    UnknownEntryArgument { name: String },
    #[error("AWBC entry argument `{name}` expected {expected}, received {actual}")]
    EntryArgumentType {
        name: String,
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
        let entry_record = program
            .entries
            .get(entry.index())
            .ok_or(FiberStateError::UnknownEntry(entry.0))?;
        let function = match &entry_record.target {
            AwbcEntryTarget::Function(function) => *function,
            AwbcEntryTarget::Routes(_) => return Err(FiberStateError::RouteSelectionRequired),
        };
        Self::for_function(program, entry, function, generation, budget_quantum)
    }

    /// Creates a root fiber after the host has selected a route/function.
    pub fn for_function(
        program: &AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        generation: u64,
        budget_quantum: u64,
    ) -> Result<Self, FiberStateError> {
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let frame = FiberFrame::new(program, function, None)?;
        Ok(Self {
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
            sources: program
                .source_plans
                .iter()
                .enumerate()
                .filter_map(|(index, _)| u32::try_from(index).ok())
                .map(|index| FiberSourceState {
                    plan: AwbcSourcePlanId(index),
                    queue: Vec::new(),
                    closed: false,
                    last_error: None,
                    overflow_count: 0,
                })
                .collect(),
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

    /// Transactionally binds the root entry arguments to parameter registers.
    ///
    /// Bindings whose names all match named parameters are treated as named
    /// arguments. Otherwise, an equally-sized list is positional. Validation
    /// completes before the live frame is mutated, so a rejected step may be
    /// retried with corrected input.
    pub fn bind_entry_arguments(
        &mut self,
        program: &AwbcProgram,
        bindings: &[RuntimeBinding],
    ) -> Result<(), FiberStateError> {
        let frame = self.active_frame()?;
        let function = program
            .functions
            .get(frame.function.index())
            .ok_or(FiberStateError::UnknownFunction(frame.function.0))?;
        let entry = program
            .entries
            .get(self.entry.index())
            .ok_or(FiberStateError::UnknownEntry(self.entry.0))?;
        if entry.signature != function.signature {
            return Err(FiberStateError::InvalidFrame);
        }
        self.bind_active_frame_arguments(program, entry.signature, bindings)
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
            return Err(FiberStateError::EntryArgumentCount {
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
                    return Err(FiberStateError::DuplicateEntryArgument {
                        name: binding.name.clone(),
                    });
                }
                let Some(position) = parameter_names
                    .iter()
                    .position(|name| name.is_some_and(|name| name == binding.name))
                else {
                    return Err(FiberStateError::UnknownEntryArgument {
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
                return Err(FiberStateError::EntryArgumentType {
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
        for (index, frame) in self.frames.iter().enumerate() {
            validate_frame(program, frame)?;
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
        for source in &self.sources {
            if program.source_plans.get(source.plan.index()).is_none() {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        for stream in &self.streams {
            if program.stream_plans.get(stream.plan.index()).is_none() {
                return Err(FiberStateError::InvalidFrame);
            }
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
        self.require_status(FiberStatus::Running)?;
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        let mut frame = FiberFrame::new(
            program,
            function,
            Some(FiberReturnPoint {
                resume: return_to,
                destination,
            }),
        )?;
        frame.bind_positional_arguments(program, args)?;
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
        let mut frame = FiberFrame::new(program, function, return_to)?;
        frame.bind_positional_arguments(program, args)?;
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
        let point = program
            .resume_points
            .get(return_to.resume.index())
            .ok_or(FiberStateError::UnknownResumePoint(return_to.resume.0))?;
        if caller.function != point.function || caller.layout != point.frame_layout {
            return Err(FiberStateError::InvalidFrame);
        }
        self.frames.pop();
        self.cursor = FiberCursor {
            function: point.function,
            block: point.block,
            instruction_offset: 0,
        };
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
        match (return_to.destination, value.as_ref()) {
            (Some(_), Some(_)) | (None, None) => {}
            _ => return Err(FiberStateError::ReturnValueMismatch),
        }
        if let Some(destination) = return_to.destination {
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
        }
        let popped = self
            .pop_call_frame(program)?
            .ok_or(FiberStateError::InvalidFrame)?;
        debug_assert_eq!(popped, return_to);
        if let (Some(destination), Some(value)) = (return_to.destination, value) {
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

    pub fn mark_trapped(&mut self, trap: FiberTrap) {
        self.status = FiberStatus::Trapped;
        self.suspension = None;
        self.terminal = Some(FiberTerminalValue::Trapped(trap));
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
        FiberStatus::Returned | FiberStatus::Trapped => {
            if state.suspension.is_some() || state.terminal.is_none() {
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
    if !function_owns_block(function, cursor.block) || cursor.instruction_offset != 0 {
        return Err(FiberStateError::InvalidFrame);
    }
    if program.blocks.get(cursor.block.index()).is_none() {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn validate_frame(program: &AwbcProgram, frame: &FiberFrame) -> Result<(), FiberStateError> {
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
        if !runtime_value_matches_type(program, value, slot.ty, 0) {
            let register = u32::try_from(index).unwrap_or(u32::MAX);
            return Err(FiberStateError::RegisterOutOfBounds {
                register,
                layout: frame.layout.0,
            });
        }
    }
    for cleanup in &frame.root_cleanups {
        validate_cleanup(program, cleanup)?;
    }
    for scope in &frame.scopes {
        if scope.depth > layout.max_scope_depth {
            return Err(FiberStateError::InvalidFrame);
        }
        for cleanup in &scope.cleanups {
            validate_cleanup(program, cleanup)?;
        }
    }
    Ok(())
}

fn validate_cleanup(
    program: &AwbcProgram,
    cleanup: &FiberScopeCleanup,
) -> Result<(), FiberStateError> {
    if cleanup.key.is_empty() || program.effect_plans.get(cleanup.effect.index()).is_none() {
        return Err(FiberStateError::InvalidFrame);
    }
    Ok(())
}

fn validate_return_point(
    program: &AwbcProgram,
    caller: &FiberFrame,
    return_to: FiberReturnPoint,
) -> Result<(), FiberStateError> {
    let point = program
        .resume_points
        .get(return_to.resume.index())
        .ok_or(FiberStateError::UnknownResumePoint(return_to.resume.0))?;
    if point.function != caller.function {
        return Err(FiberStateError::ResumeFunctionMismatch {
            resume: return_to.resume.0,
            actual: point.function.0,
            expected: caller.function.0,
        });
    }
    if point.frame_layout != caller.layout {
        return Err(FiberStateError::ResumeLayoutMismatch {
            resume: return_to.resume.0,
            actual: point.frame_layout.0,
            expected: caller.layout.0,
        });
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

fn validate_suspension(
    program: &AwbcProgram,
    state: &FiberState,
    suspension: &FiberSuspension,
) -> Result<(), FiberStateError> {
    let frame = state.active_frame()?;
    validate_return_point(
        program,
        frame,
        FiberReturnPoint {
            resume: suspension.resume,
            destination: None,
        },
    )?;
    match &suspension.reason {
        FiberSuspensionReason::Dialogue {
            content,
            line_task_group,
        } => {
            if program.content_units.get(content.index()).is_none()
                || program
                    .line_task_groups
                    .get(line_task_group.index())
                    .is_none()
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
        FiberSuspensionReason::Await { binding, .. } => {
            if let Some(binding) = binding
                && program.patterns.get(binding.index()).is_none()
            {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        FiberSuspensionReason::AwaitMany(await_many) => {
            if program.task_plans.get(await_many.plan.index()).is_none() {
                return Err(FiberStateError::InvalidFrame);
            }
            if let Some(binding) = await_many.binding
                && program.patterns.get(binding.index()).is_none()
            {
                return Err(FiberStateError::InvalidFrame);
            }
            if await_many.results.len() > await_many.items.len() {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        FiberSuspensionReason::HostCall { call, .. } => {
            if program.host_calls.get(call.index()).is_none() {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        FiberSuspensionReason::BudgetYield => {}
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
            return Err(FiberStateError::EntryArgumentCount {
                expected: parameters.len(),
                actual: args.len(),
            });
        }
        let mut next = self.registers.clone();
        for (position, ((register, slot), value)) in parameters.iter().zip(args).enumerate() {
            let expected = signature.params[position];
            if slot.ty != expected || !runtime_value_matches_type(program, value, expected, 0) {
                return Err(FiberStateError::EntryArgumentType {
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
}

fn runtime_value_matches_type(
    program: &AwbcProgram,
    value: &RuntimeValue,
    ty: AwbcTypeId,
    depth: usize,
) -> bool {
    if depth > 64 {
        return false;
    }
    let Some(ty) = program.runtime_types.get(ty.index()) else {
        return false;
    };
    match (value, ty) {
        (_, AwbcRuntimeType::Dynamic)
        | (
            RuntimeValue::String(_),
            AwbcRuntimeType::String | AwbcRuntimeType::TaskHandle | AwbcRuntimeType::NeedHandle,
        )
        | (RuntimeValue::Unit, AwbcRuntimeType::Unit)
        | (RuntimeValue::Bool(_), AwbcRuntimeType::Bool)
        | (RuntimeValue::F32(_), AwbcRuntimeType::F32)
        | (RuntimeValue::F64(_), AwbcRuntimeType::F64)
        | (RuntimeValue::Char(_), AwbcRuntimeType::Char)
        | (RuntimeValue::Duration(_), AwbcRuntimeType::Duration)
        | (RuntimeValue::EntityRef(_), AwbcRuntimeType::EntityRef)
        | (RuntimeValue::MatrixF32(_), AwbcRuntimeType::MatrixF32)
        | (RuntimeValue::MatrixF64(_), AwbcRuntimeType::MatrixF64)
        | (RuntimeValue::TensorF32(_), AwbcRuntimeType::TensorF32)
        | (RuntimeValue::TensorF64(_), AwbcRuntimeType::TensorF64) => true,
        (RuntimeValue::Int(value), AwbcRuntimeType::Int(kind)) => signed_kind(*value) == *kind,
        (RuntimeValue::UInt(value), AwbcRuntimeType::UInt(kind)) => unsigned_kind(*value) == *kind,
        (RuntimeValue::Tuple(values), AwbcRuntimeType::Tuple(types)) => {
            values.len() == types.len()
                && values
                    .iter()
                    .zip(types)
                    .all(|(value, ty)| runtime_value_matches_type(program, value, *ty, depth + 1))
        }
        (RuntimeValue::Seq(values), AwbcRuntimeType::Sequence(item)) => values
            .clone()
            .into_values()
            .iter()
            .all(|value| runtime_value_matches_type(program, value, *item, depth + 1)),
        (RuntimeValue::Record(values), AwbcRuntimeType::Record { fields, .. }) => {
            values.len() == fields.len()
                && values.iter().zip(fields).all(|(value, field)| {
                    runtime_value_matches_type(program, &value.value, field.ty, depth + 1)
                })
        }
        (RuntimeValue::Variant { name, payload, .. }, AwbcRuntimeType::Variant { cases, .. }) => {
            cases.iter().any(|case| {
                program
                    .strings
                    .get(case.name.index())
                    .is_some_and(|case_name| {
                        case_name == name
                            && match (case.payload, payload.as_deref()) {
                                (None, None) => true,
                                (Some(ty), Some(value)) => {
                                    runtime_value_matches_type(program, value, ty, depth + 1)
                                }
                                _ => false,
                            }
                    })
            })
        }
        _ => false,
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
        RuntimeValue::Range(_) => "range",
        RuntimeValue::Iterator(_) => "iterator",
        RuntimeValue::EntityRef(_) => "entity",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::Seq(_) => "sequence",
        RuntimeValue::Record(_) => "record",
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
        AwbcBlock, AwbcEntry, AwbcEntryKind, AwbcFrameLayout, AwbcFrameSlot, AwbcFunction,
        AwbcFunctionFlags, AwbcFunctionKind, AwbcSafePointKind, AwbcSignature, AwbcStringId,
        AwbcTableRange, AwbcTerminator,
    };

    fn entry_arguments_program() -> AwbcProgram {
        let mut program = AwbcProgram::default();
        program.strings = vec!["a".to_owned(), "b".to_owned(), "entry".to_owned()];
        let string_ty = AwbcTypeId(u32::try_from(program.runtime_types.len()).unwrap());
        program.runtime_types.push(AwbcRuntimeType::String);
        let i64_ty = AwbcTypeId(u32::try_from(program.runtime_types.len()).unwrap());
        program
            .runtime_types
            .push(AwbcRuntimeType::Int(AwbcSignedIntKind::I64));
        program.signatures.push(AwbcSignature {
            params: vec![string_ty, i64_ty],
            result: None,
            effects: Default::default(),
        });
        program.frame_layouts.push(AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: Some(AwbcStringId(0)),
                    ty: string_ty,
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: Some(AwbcStringId(1)),
                    ty: i64_ty,
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        });
        program.functions.push(AwbcFunction {
            public_id: Some(AwbcStringId(2)),
            kind: AwbcFunctionKind::Flow,
            signature: Default::default(),
            frame_layout: Default::default(),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: Default::default(),
            flags: AwbcFunctionFlags::default(),
        });
        program.blocks.push(AwbcBlock {
            owner: Default::default(),
            instructions: AwbcTableRange::default(),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::Return,
            source_map: None,
        });
        program.entries.push(AwbcEntry {
            public_id: AwbcStringId(2),
            kind: AwbcEntryKind::Game,
            signature: Default::default(),
            target: AwbcEntryTarget::Function(Default::default()),
        });
        program
    }

    fn binding(name: &str, value: RuntimeValue) -> RuntimeBinding {
        RuntimeBinding {
            name: name.to_owned(),
            value,
        }
    }

    #[test]
    fn entry_arguments_accept_positional_and_named_equivalent_bindings() {
        let program = entry_arguments_program();
        let mut positional = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        positional
            .bind_entry_arguments(
                &program,
                &[
                    binding("$0", RuntimeValue::String("value".to_owned())),
                    binding("$1", RuntimeValue::i64(7)),
                ],
            )
            .unwrap();
        assert_eq!(
            positional
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(0)),
            Ok(&RuntimeValue::String("value".to_owned()))
        );
        assert_eq!(
            positional
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(1)),
            Ok(&RuntimeValue::i64(7))
        );

        let mut named = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        named
            .bind_entry_arguments(
                &program,
                &[
                    binding("b", RuntimeValue::i64(11)),
                    binding("a", RuntimeValue::String("named".to_owned())),
                ],
            )
            .unwrap();
        assert_eq!(
            named.active_frame().unwrap().register(AwbcRegisterId(0)),
            Ok(&RuntimeValue::String("named".to_owned()))
        );
        assert_eq!(
            named.active_frame().unwrap().register(AwbcRegisterId(1)),
            Ok(&RuntimeValue::i64(11))
        );
    }

    #[test]
    fn rejected_entry_arguments_are_transactional_and_retryable() {
        let program = entry_arguments_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        let before = fiber.clone();
        let error = fiber
            .bind_entry_arguments(
                &program,
                &[
                    binding("a", RuntimeValue::String("ok".to_owned())),
                    binding("b", RuntimeValue::Bool(false)),
                ],
            )
            .unwrap_err();
        assert!(matches!(error, FiberStateError::EntryArgumentType { .. }));
        assert_eq!(fiber, before);

        fiber
            .bind_entry_arguments(
                &program,
                &[
                    binding("a", RuntimeValue::String("ok".to_owned())),
                    binding("b", RuntimeValue::i64(9)),
                ],
            )
            .unwrap();
        assert_eq!(
            fiber.active_frame().unwrap().register(AwbcRegisterId(1)),
            Ok(&RuntimeValue::i64(9))
        );
    }

    #[test]
    fn entry_arguments_reject_missing_unknown_and_duplicate_names() {
        let program = entry_arguments_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        assert!(matches!(
            fiber.bind_entry_arguments(
                &program,
                &[binding("a", RuntimeValue::String("value".to_owned()))]
            ),
            Err(FiberStateError::EntryArgumentCount { .. })
        ));
        assert!(matches!(
            fiber.bind_entry_arguments(
                &program,
                &[
                    binding("a", RuntimeValue::String("value".to_owned())),
                    binding("missing", RuntimeValue::i64(1)),
                ]
            ),
            Err(FiberStateError::UnknownEntryArgument { .. })
        ));
        assert!(matches!(
            fiber.bind_entry_arguments(
                &program,
                &[
                    binding("a", RuntimeValue::String("value".to_owned())),
                    binding("a", RuntimeValue::i64(1)),
                ]
            ),
            Err(FiberStateError::DuplicateEntryArgument { .. })
        ));
    }

    #[test]
    fn fiber_snapshot_validation_rejects_invalid_cursor_shape() {
        let program = entry_arguments_program();
        let mut fiber = FiberState::for_entry(&program, Default::default(), 0, 64).unwrap();
        fiber.validate_for_program(&program).unwrap();

        fiber.cursor.instruction_offset = 1;
        assert_eq!(
            fiber.validate_for_program(&program),
            Err(FiberStateError::InvalidFrame)
        );
    }
}
