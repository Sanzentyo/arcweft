//! Executor-neutral AWBC fiber and safe-point state.

use super::schema::{
    AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcEffectPlanId, AwbcEntryId, AwbcEntryTarget,
    AwbcFrameLayoutId, AwbcFrameSlotRole, AwbcFunctionId, AwbcHostCallId, AwbcLineTaskGroupId,
    AwbcPatternId, AwbcProgram, AwbcRegisterId, AwbcResumePointId, AwbcRuntimeType, AwbcScopeId,
    AwbcSignatureId, AwbcSignedIntKind, AwbcSourceMapId, AwbcSourcePlanId, AwbcStreamPlanId,
    AwbcTaskPlanId, AwbcTrapCode, AwbcTypeId, AwbcUnsignedIntKind, AwbcVariantIdentity,
};
use crate::entry::RuntimeNominalTypeId;
use crate::pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity};
use crate::task::NeedId;
use crate::value::{
    RuntimeBinding, RuntimeFunctionBody, RuntimeFunctionValue, RuntimeInt, RuntimeIterator,
    RuntimeSeq, RuntimeUInt, RuntimeValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
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
    /// Offset of the next instruction, or the block length when its terminator is next.
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
        line_task_group: AwbcLineTaskGroupId,
    },
    Choice {
        choice: AwbcChoiceId,
        destination: AwbcRegisterId,
    },
    Await {
        target: FiberAwaitTarget,
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
    Cancelled,
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
    #[error("structured expression function bodies are not valid in an AWBC fiber")]
    StructuredRuntimeFunction,
    #[error("invalid AWBC runtime function: {reason}")]
    InvalidRuntimeFunction { reason: String },
    #[error("invalid runtime value at {path}: {reason}")]
    InvalidRuntimeValue { path: String, reason: String },
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
        if let Some(active_frame) = self.frames.last()
            && active_frame.function != self.cursor.function
        {
            return Err(FiberStateError::InvalidFrame);
        }
        for (index, frame) in self.frames.iter().enumerate() {
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
        for (index, source) in self.sources.iter().enumerate() {
            validate_source(program, source, &format!("sources[{index}]"))?;
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
        let mut frame = FiberFrame::new(program, function, Some(return_to))?;
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
            .try_for_each(|field| validate_nested_runtime_value(program, &field.value, depth + 1)),
        RuntimeValue::NominalRecord(record) => record
            .fields()
            .iter()
            .try_for_each(|field| validate_nested_runtime_value(program, field, depth + 1)),
        RuntimeValue::Opaque(value) => {
            validate_nested_runtime_value(program, value.payload(), depth + 1)
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
            validate_nested_runtime_sequence(program, &field.values, depth + 1)
        }),
        RuntimeSeq::Dense(_) => Ok(()),
    }
}

fn validate_runtime_function(
    program: &AwbcProgram,
    function: &RuntimeFunctionValue,
    depth: usize,
) -> Result<(), FiberStateError> {
    let RuntimeFunctionBody::Awbc(function_id) = &function.body else {
        return Err(FiberStateError::StructuredRuntimeFunction);
    };
    let function_id = *function_id;
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
    let stored_arity = function
        .captures
        .len()
        .saturating_add(function.params.len());
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

    let stored_names = function
        .captures
        .iter()
        .map(|capture| capture.name.as_str())
        .chain(function.params.iter().map(String::as_str))
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
    for (position, capture) in function.captures.iter().enumerate() {
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
        FiberSuspensionReason::Await { target, binding } => {
            validate_await_suspension(program, target, *binding)?;
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

fn validate_source(
    program: &AwbcProgram,
    source: &FiberSourceState,
    path: &str,
) -> Result<(), FiberStateError> {
    let plan = program
        .source_plans
        .get(source.plan.index())
        .ok_or(FiberStateError::InvalidFrame)?;
    for (index, value) in source.queue.iter().enumerate() {
        validate_runtime_value_at(
            program,
            value,
            Some(plan.item_type),
            format!("{path}.queue[{index}]"),
        )?;
    }
    if let Some(error) = &source.last_error {
        validate_runtime_value_at(
            program,
            error,
            Some(plan.error_type),
            format!("{path}.last_error"),
        )?;
    }
    Ok(())
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

pub(crate) fn runtime_value_matches_type(
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
        (
            RuntimeValue::Variant {
                owner: actual_owner,
                ordinal,
                name,
                payload,
            },
            AwbcRuntimeType::Variant { owner, cases },
        ) => {
            runtime_variant_identity(program, owner).as_ref() == Some(actual_owner)
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
        (value, AwbcRuntimeType::Choice(alternatives)) => alternatives
            .iter()
            .any(|alternative| runtime_value_matches_type(program, value, *alternative, depth + 1)),
        (
            RuntimeValue::NominalRecord(record),
            AwbcRuntimeType::Nominal {
                public_id, layout, ..
            },
        ) => {
            program
                .strings
                .get(public_id.index())
                .is_some_and(|expected| record.type_id().as_str() == expected)
                && record.layout().as_bytes() == layout
        }
        _ => false,
    }
}

pub(crate) fn runtime_variant_identity(
    program: &AwbcProgram,
    owner: &AwbcVariantIdentity,
) -> Option<RuntimeVariantIdentity> {
    match owner {
        AwbcVariantIdentity::Nominal {
            public_id,
            semantic_identity,
        } => Some(RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new(program.strings.get(public_id.index())?.clone())
                .ok()?,
            semantic_identity: RuntimeSemanticTypeId::from_bytes(*semantic_identity),
        }),
        AwbcVariantIdentity::Option => Some(RuntimeVariantIdentity::Option),
        AwbcVariantIdentity::Result => Some(RuntimeVariantIdentity::Result),
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
        RuntimeValue::NominalRecord(record) => record.type_id().as_str(),
        RuntimeValue::Opaque(_) => "opaque value",
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
        AwbcFunction, AwbcFunctionFlags, AwbcFunctionKind, AwbcSafePointKind, AwbcSignature,
        AwbcStringId, AwbcTableRange, AwbcTerminator,
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
            public_id: AwbcStringId(2),
            kind: AwbcEntryKind::Cli,
            signature: Default::default(),
            target: AwbcEntryTarget::Function(Default::default()),
            roles: crate::entry::RuntimeEntryRoles::None,
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
