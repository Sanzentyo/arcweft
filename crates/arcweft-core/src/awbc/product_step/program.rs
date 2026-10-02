//! Owned admitted-program activation and single-owner result transfer.

use std::sync::Arc;

use arcweft_id::runtime_program::RuntimePureProgramId;

use super::{AwbcProductStepBuildError, AwbcProductStepExecutor};
use crate::awbc::fiber::{
    AwbcFiberRoot, FiberState, FiberTerminalValue, validate_function_argument_values,
};
use crate::awbc::schema::AwbcProgram;
use crate::task::GenerationId;
use crate::value::RuntimeValue;

/// No inputs leave the caller's retained packet when activation is rejected.
#[derive(Debug)]
pub struct AwbcProgramInvocationError {
    reason: AwbcProductStepBuildError,
    inputs: Vec<RuntimeValue>,
}

impl AwbcProgramInvocationError {
    pub fn into_parts(self) -> (AwbcProductStepBuildError, Vec<RuntimeValue>) {
        (self.reason, self.inputs)
    }
}

impl AwbcProductStepExecutor {
    /// Preflights a complete program ABI before transferring inputs into its
    /// verified frame. Execution and backend calls happen only during stepping.
    pub fn for_program_invocation(
        program: Arc<AwbcProgram>,
        id: RuntimePureProgramId,
        inputs: Vec<RuntimeValue>,
        generation: GenerationId,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProgramInvocationError> {
        let prepared = (|| {
            let binding = program.pure_program_binding(id).ok_or_else(|| {
                AwbcProductStepBuildError::FiberState {
                    message: format!("admitted program {id} is absent"),
                }
            })?;
            let inputs = validate_function_argument_values(&program, binding.function, &inputs)
                .map_err(|error| AwbcProductStepBuildError::FiberState {
                    message: error.to_string(),
                })?;
            let executor = Self::for_root_arc_with_context_proof(
                Arc::clone(&program),
                AwbcFiberRoot::Program(id),
                budget_quantum,
                generation,
                None,
            )?;
            Ok::<_, AwbcProductStepBuildError>((executor, inputs))
        })();
        let (mut executor, prepared) = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => return Err(AwbcProgramInvocationError { reason, inputs }),
        };
        executor.fiber = FiberState::for_function_with_arguments_prepared(
            &program,
            AwbcFiberRoot::Program(id),
            inputs,
            prepared,
            executor.fiber.instance,
            generation.get(),
            budget_quantum.max(1),
        );
        Ok(executor)
    }

    /// Moves a completed value to the caller once; the terminal label remains
    /// available to status and snapshots without duplicating the live value.
    pub fn take_program_result(&mut self) -> Option<(RuntimePureProgramId, RuntimeValue)> {
        let AwbcFiberRoot::Program(id) = self.fiber.root else {
            return None;
        };
        let Some(FiberTerminalValue::Returned(value)) = &mut self.fiber.terminal else {
            return None;
        };
        value.take().map(|value| (id, value))
    }
}
