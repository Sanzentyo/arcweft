//! A private child frame whose Copy inputs are prepared and whose Move inputs
//! remain in the parent until its genuine Source receipt has been accepted.

use super::*;
use crate::value::RuntimeLocalReadMode;

pub(crate) struct PreparedOwnedProgramTaskFiber {
    fiber: FiberState,
    parameters: Box<[AwbcRegisterId]>,
}

impl PreparedOwnedProgramTaskFiber {
    pub(crate) fn bind(mut self, values: Vec<RuntimeValue>) -> FiberState {
        assert_eq!(values.len(), self.parameters.len());
        for (value, register) in values.into_iter().zip(self.parameters) {
            assert!(
                self.fiber.frames[0].registers[register.index()]
                    .as_ref()
                    .is_none()
            );
            self.fiber.frames[0].registers[register.index()] = value.into();
        }
        self.fiber
    }
}

impl PreparedFunctionInputBinding {
    pub(crate) fn prepare_owned_program_task_fiber(
        self,
        program: &AwbcProgram,
        task: crate::awbc::schema::AwbcTaskPlanId,
        instance: RuntimeFiberInstanceId,
        generation: u64,
        quantum: u64,
    ) -> Result<PreparedOwnedProgramTaskFiber, FiberStateError> {
        let Some(crate::awbc::schema::AwbcTaskPlanExecution::Program(body)) = program
            .task_plans
            .get(task.index())
            .map(|row| &row.execution)
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        if self.function != body.kernel {
            return Err(FiberStateError::InvalidFrame);
        }
        let (fiber, parameters) = self.allocate_fiber(
            program,
            AwbcFiberRoot::Task(task),
            instance,
            generation,
            quantum.max(1),
        );
        Ok(PreparedOwnedProgramTaskFiber { fiber, parameters })
    }
}

pub(crate) struct PreparedProgramTaskFiber {
    parent: RuntimeFiberInstanceId,
    frame: RuntimeFrameInstanceId,
    child: FiberState,
    moves: Vec<(AwbcRegisterId, AwbcRegisterId)>,
}

impl PreparedProgramTaskFiber {
    /// The product owns the parent exclusively from preparation through commit;
    /// registry admission cannot mutate its frame or advance its instruction.
    pub(crate) fn commit(mut self, parent: &mut FiberState) -> FiberState {
        assert_eq!(parent.instance, self.parent);
        let frame = parent
            .active_frame_mut()
            .expect("prepared parent remains active");
        assert_eq!(frame.instance, self.frame);
        for (source, destination) in &self.moves {
            assert!(frame.register(*source).is_ok());
            assert!(
                self.child.frames[0].registers[destination.index()]
                    .as_ref()
                    .is_none()
            );
        }
        for (source, destination) in self.moves {
            self.child.frames[0].registers[destination.index()] = frame
                .take_register(source)
                .expect("every admitted unique Move remains in the exclusive parent")
                .into();
        }
        self.child
    }
}

impl FiberState {
    pub(crate) fn prepare_program_task_fiber(
        &self,
        program: &AwbcProgram,
        task: crate::awbc::schema::AwbcTaskPlanId,
        captures: &[AwbcRegisterId],
        inputs: PreparedFunctionInputBinding,
        instance: RuntimeFiberInstanceId,
        generation: u64,
    ) -> Result<PreparedProgramTaskFiber, FiberStateError> {
        let Some(crate::awbc::schema::AwbcTaskPlanExecution::Program(body)) = program
            .task_plans
            .get(task.index())
            .map(|row| &row.execution)
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        if inputs.function != body.kernel {
            return Err(FiberStateError::InvalidFrame);
        }
        let modes = &body.capture_modes;
        if captures.len() != modes.len() || captures.len() != inputs.parameter_registers.len() {
            return Err(FiberStateError::ArgumentCount {
                expected: inputs.parameter_registers.len(),
                actual: captures.len(),
            });
        }
        let frame = self.active_frame()?;
        let mut moved = std::collections::BTreeSet::new();
        for (position, (source, mode)) in captures.iter().zip(modes).enumerate() {
            let value = frame.register(*source)?;
            match mode {
                RuntimeLocalReadMode::Copy if !value.ownership().permits_copy() => {
                    return Err(FiberStateError::ProgramCaptureCopy { position });
                }
                RuntimeLocalReadMode::Move if !moved.insert(*source) => {
                    return Err(FiberStateError::DuplicateProgramCaptureMove { register: *source });
                }
                _ => {}
            }
        }
        let (mut child, parameters) = inputs.allocate_fiber(
            program,
            AwbcFiberRoot::Task(task),
            instance,
            generation,
            self.budget.quantum.max(1),
        );
        let mut moves = Vec::with_capacity(moved.len());
        for ((source, mode), destination) in captures.iter().zip(modes).zip(parameters) {
            match mode {
                RuntimeLocalReadMode::Copy => {
                    child.frames[0].registers[destination.index()] = frame
                        .register(*source)
                        .expect("all capture reads were admitted before copying")
                        .clone()
                        .into()
                }
                RuntimeLocalReadMode::Move => moves.push((*source, destination)),
            }
        }
        Ok(PreparedProgramTaskFiber {
            parent: self.instance,
            frame: frame.instance,
            child,
            moves,
        })
    }
}
