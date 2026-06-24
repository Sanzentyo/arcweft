//! Executor-neutral AWBC fiber and safe-point state.

use super::schema::{
    AwbcBlockId, AwbcChoiceId, AwbcContentUnitId, AwbcEntryId, AwbcEntryTarget, AwbcFrameLayoutId,
    AwbcFunctionId, AwbcHostCallId, AwbcLineTaskGroupId, AwbcPatternId, AwbcProgram,
    AwbcRegisterId, AwbcResumePointId, AwbcScopeId, AwbcSourceMapId, AwbcSourcePlanId,
    AwbcStreamPlanId, AwbcTaskPlanId, AwbcTrapCode,
};
use crate::value::RuntimeValue;
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
    pub scopes: Vec<FiberScope>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberReturnPoint {
    pub resume: AwbcResumePointId,
    pub destination: Option<AwbcRegisterId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FiberScope {
    pub id: AwbcScopeId,
    pub depth: u32,
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

    pub fn checkpoint(&self) -> FiberCheckpoint {
        FiberCheckpoint {
            state: Box::new(self.clone()),
        }
    }

    pub fn restore(&mut self, checkpoint: FiberCheckpoint) {
        *self = *checkpoint.state;
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
        self.require_status(FiberStatus::Running)?;
        let function_record = program
            .functions
            .get(function.index())
            .ok_or(FiberStateError::UnknownFunction(function.0))?;
        self.frames.push(FiberFrame::new(
            program,
            function,
            Some(FiberReturnPoint {
                resume: return_to,
                destination,
            }),
        )?);
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
            scopes: Vec::with_capacity(layout.max_scope_depth as usize),
        })
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
