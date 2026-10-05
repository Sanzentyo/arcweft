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
    /// Continues with the complete existing executor context. The previous
    /// result supplies one positional input without leaving its ledger owner.
    pub fn continue_program(
        mut self,
        id: RuntimePureProgramId,
        inputs: Vec<crate::program_invocation::RuntimeProgramInput>,
    ) -> Result<Self, crate::program_invocation::RuntimeProgramContinuationError<Self>> {
        use crate::program_invocation::{
            RuntimeProgramContinuationError, RuntimeProgramContinuationFailure,
            RuntimeProgramInputRollback, input_refs, into_values,
        };
        let graph_error = |error: super::ProductStepError| match error {
            super::ProductStepError::Line(error) => {
                RuntimeProgramContinuationFailure::Custody(error)
            }
            error => {
                RuntimeProgramContinuationFailure::Awbc(AwbcProductStepBuildError::FiberState {
                    message: error.to_string(),
                })
            }
        };
        let prepared = (|| {
            if !self.fiber.program_continuation_ready()
                || !self.child_fibers.is_empty()
                || self.root.is_some()
                || self.active_choice.is_some()
                || self.pending_host_call.is_some()
            {
                return Err(RuntimeProgramContinuationFailure::NotCompleted);
            }
            let Some(FiberTerminalValue::Returned(Some(result))) = &self.fiber.terminal else {
                return Err(RuntimeProgramContinuationFailure::NotCompleted);
            };
            if !matches!(self.fiber.root, AwbcFiberRoot::Program(_)) {
                return Err(RuntimeProgramContinuationFailure::NotCompleted);
            }
            let refs = input_refs(&inputs, result)?;
            let binding = self.program.pure_program_binding(id).ok_or_else(|| {
                AwbcProductStepBuildError::FiberState {
                    message: format!("admitted program {id} is absent"),
                }
            })?;
            let layout = crate::awbc::fiber::validate_function_argument_value_refs(
                &self.program,
                binding.function,
                &refs,
            )?;
            let before =
                super::line::product_fiber_handle_owners(self.facade_fiber.execution, &self.fiber)
                    .map_err(graph_error)?;
            self.dialogues.inspect_parent_fiber_reconciliation(
                self.facade_fiber.execution,
                &before,
                &before,
                &Default::default(),
            )?;
            let image = self.inert_rollback_image().map_err(|error| {
                RuntimeProgramContinuationFailure::Snapshot {
                    message: error.to_string(),
                }
            })?;
            let owner = crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program));
            let input_image = RuntimeProgramInputRollback::capture(&inputs, &owner)?;
            let mut next_frame_instance = self.fiber.next_frame_instance;
            let frame_instance = crate::runtime_id::RuntimeFrameInstanceId::from_allocated(
                next_frame_instance
                    .take_next(crate::runtime_id::RuntimeIdNamespace::FrameInstance)
                    .map_err(crate::awbc::fiber::FiberStateError::from)?,
            );
            Ok::<_, RuntimeProgramContinuationFailure>((
                layout,
                before,
                image,
                owner,
                input_image,
                frame_instance,
                next_frame_instance,
            ))
        })();
        let (layout, before, image, owner, input_image, frame_instance, next_frame_instance) =
            match prepared {
                Ok(prepared) => prepared,
                Err(reason) => {
                    return Err(RuntimeProgramContinuationError {
                        reason,
                        executor: Box::new(self),
                        inputs,
                    });
                }
            };
        let custody = match std::mem::take(&mut self.dialogues).into_published() {
            Ok(custody) => custody,
            Err((store, reason)) => {
                self.dialogues = store;
                return Err(RuntimeProgramContinuationError {
                    reason: reason.into(),
                    executor: Box::new(self),
                    inputs,
                });
            }
        };
        self.dialogues = super::dialogue::ProductDialogueStore::from_published(custody);
        let old_origin = self.fiber.root;
        let old_generation = self.runtime_generation;
        let old_quantum = self.fiber.budget.quantum;
        let Some(FiberTerminalValue::Returned(Some(result))) = self.fiber.terminal.take() else {
            unreachable!("the complete borrowed continuation proof retains its result")
        };
        let streams = std::mem::take(&mut self.fiber.streams);
        let line_cursor = self.fiber.line_cursor;
        let budget = self.fiber.budget;
        self.fiber = FiberState::for_function_with_arguments_prepared(
            &self.program,
            AwbcFiberRoot::Program(id),
            into_values(inputs, result),
            layout,
            self.fiber.instance,
            self.runtime_generation.get(),
            self.fiber.budget.quantum,
        );
        self.facade_fiber.status = crate::engine::FlowFiberStatus::Running;
        self.fiber.frames[0].instance = frame_instance;
        self.fiber.next_frame_instance = next_frame_instance;
        self.fiber.streams = streams;
        self.fiber.line_cursor = line_cursor;
        self.fiber.budget = budget;
        let committed = (|| {
            let after =
                super::line::product_fiber_handle_owners(self.facade_fiber.execution, &self.fiber)
                    .map_err(graph_error)?;
            let receipt = self.dialogues.reconcile_parent_fiber(
                self.facade_fiber.execution,
                &before,
                &after,
                &Default::default(),
            )?;
            assert!(
                receipt.into_commands().is_empty(),
                "program activation transfers ownership without dropping resources"
            );
            Ok::<_, RuntimeProgramContinuationFailure>(())
        })();
        if let Err(reason) = committed {
            let program = Arc::clone(&self.program);
            let format_context = self.format_context.clone();
            let plain_text_proof = self.plain_text_context_template_proof;
            drop(self);
            let mut executor = Self::for_root_arc_with_context_proof(
                program,
                old_origin,
                old_quantum,
                old_generation,
                plain_text_proof,
            )
            .expect("the unchanged original program configuration remains constructible");
            executor.format_context = format_context;
            executor
                .restore_rollback_image(image)
                .expect("the original executor restores after the candidate owner is dropped");
            return Err(RuntimeProgramContinuationError {
                reason,
                executor: Box::new(executor),
                inputs: RuntimeProgramInputRollback::restore(input_image, &owner),
            });
        }
        Ok(self)
    }

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
            for (position, value) in inputs.iter().enumerate() {
                value.validate_detached_custody().map_err(|source| {
                    AwbcProductStepBuildError::ProgramInputCustody {
                        program: id,
                        position,
                        source,
                    }
                })?;
            }
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
    pub fn take_program_result(
        &mut self,
    ) -> Result<
        Option<(RuntimePureProgramId, RuntimeValue)>,
        crate::value::ownership::RuntimeDetachedValueError,
    > {
        let AwbcFiberRoot::Program(id) = self.fiber.root else {
            return Ok(None);
        };
        let Some(FiberTerminalValue::Returned(value)) = &mut self.fiber.terminal else {
            return Ok(None);
        };
        if let Some(value) = value.as_ref() {
            value.validate_detached_custody()?;
        }
        Ok(value.take().map(|value| (id, value)))
    }
}
