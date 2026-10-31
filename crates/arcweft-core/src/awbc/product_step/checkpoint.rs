//! Private rollback owns the selected fiber, its banks and original admission.

use super::dialogue::ProductDialogueTransaction;
use super::{
    AwbcProductProgram, AwbcProductStepExecutor, ProductChildFiberOwner, ProductStepError,
};
use crate::awbc::fiber::{AwbcFiberStateSnapshot, FiberState, FiberStateError};
use crate::presentation::resources::{PresentationBanks, PresentationBody};
use crate::runtime_id::RuntimePresentationBankId;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    AwbcRuntimeValueSnapshot, RuntimePayload, RuntimeRetainedValueAdmission,
    RuntimeValueRestoreContext,
};

/// Coordinates of actual retained execution owners, never reconstructed frames.
#[derive(Clone, Copy)]
pub(super) enum ProductCheckpointTarget<'a> {
    Main,
    Child(usize),
    Dialogue(&'a ProductDialogueTransaction),
}

impl<'a> ProductCheckpointTarget<'a> {
    fn transaction(self) -> Option<&'a ProductDialogueTransaction> {
        match self {
            Self::Dialogue(transaction) => Some(transaction),
            Self::Main | Self::Child(_) => None,
        }
    }

    fn fiber(
        self,
        executor: &'a AwbcProductStepExecutor,
    ) -> Result<&'a FiberState, FiberStateError> {
        match self {
            Self::Main => Some(&executor.fiber),
            Self::Child(index) => executor.child_fibers.get(index).map(|child| &child.fiber),
            Self::Dialogue(transaction) => transaction.frame().activation_fiber(),
        }
        .ok_or(FiberStateError::InvalidFrame)
    }
}

/// No Clone or wire reader: the complete original admission is consumed by a
/// single restore. The inert images use the existing owning grammar.
pub(super) struct ProductExecutionCheckpoint {
    fiber: AwbcFiberStateSnapshot,
    presentation: Option<PresentationBanks<PresentationBody<AwbcRuntimeValueSnapshot>>>,
    admission: RuntimeRetainedValueAdmission,
}

impl ProductExecutionCheckpoint {
    pub(super) fn restore(
        self,
        fiber: &mut FiberState,
        presentation: &mut Option<PresentationBanks<PresentationBody<RuntimePayload>>>,
        program: &AwbcProductProgram,
    ) -> Result<(), FiberStateError> {
        let owner = self
            .admission
            .program_owner()
            .ok_or(FiberStateError::CheckpointProgramOwner)?;
        if !owner.same_program(&RuntimeProgramOwner::Awbc(program.program_arc()))
            || self.admission.task_source() != program.original_task_source()
            || self.fiber.instance != fiber.instance
            || self.fiber.presentation_bank() != fiber.presentation_bank()
        {
            return Err(FiberStateError::CheckpointProgramOwner);
        }
        let context = RuntimeValueRestoreContext::new(
            owner,
            self.admission
                .task_source()
                .map(|source| crate::task::TaskSourceRestore {
                    program: source,
                    admission: &self.admission,
                }),
        );
        let candidate = self.fiber.into_admitted_live_for_program(context)?;
        let banks = self
            .presentation
            .map(|banks| banks.into_admitted_live(context))
            .transpose()?;
        *fiber = candidate;
        *presentation = banks;
        Ok(())
    }
}

impl AwbcProductStepExecutor {
    pub(super) fn execution_checkpoint<'a>(
        &'a self,
        target: ProductCheckpointTarget<'a>,
    ) -> Result<ProductExecutionCheckpoint, ProductStepError> {
        let admission = self.retained_execution_admission(target)?;
        let owner = admission
            .program_owner()
            .expect("Product admission retains its original program");
        let fiber = AwbcFiberStateSnapshot::from_admitted_live(target.fiber(self)?, owner)?;
        let presentation = self
            .presentation
            .as_ref()
            .map(|banks| banks.snapshot_admitted())
            .transpose()?;
        Ok(ProductExecutionCheckpoint {
            fiber,
            presentation,
            admission,
        })
    }

    /// Completes the actual retained contour before any checkpoint or snapshot
    /// copies values. A withdrawn Dialogue participates through its reservation.
    pub(super) fn retained_execution_admission<'a>(
        &'a self,
        target: ProductCheckpointTarget<'a>,
    ) -> Result<RuntimeRetainedValueAdmission, ProductStepError> {
        if self.fiber.presentation_bank() != RuntimePresentationBankId::Root {
            return Err(ProductStepError::Internal(
                "Product main fiber must retain the Root presentation bank".into(),
            ));
        }
        self.fiber.root.admit_executor_roots(
            self.root.as_ref(),
            &self.program,
            self.child_fibers.iter().map(|child| {
                (
                    child.fiber.root,
                    child.fiber.status,
                    matches!(child.owner, ProductChildFiberOwner::Independent),
                    matches!(
                        child
                            .fiber
                            .suspension
                            .as_ref()
                            .map(|suspension| &suspension.reason),
                        Some(crate::awbc::fiber::FiberSuspensionReason::BudgetYield)
                    ),
                )
            }),
        )?;
        let mut admission = self
            .program
            .retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        let dialogues = self
            .dialogues
            .retained_contour(target.transaction().into_iter(), &mut admission)?;
        self.admit_presentation_execution_in(&dialogues, &mut admission)?;
        let owner = RuntimeProgramOwner::Awbc(self.program_arc());
        if let Some(banks) = &self.presentation {
            banks.admit_retained(&owner, self.artifact_fingerprint, &mut admission)?;
        }
        if let Some(root) = &self.root {
            root.admit_retained(&mut admission)?;
        }
        self.admit_fiber_state(&mut admission)?;
        self.admit_task_state(&mut admission)?;
        dialogues.admit_retained(&mut admission)?;
        dialogues.try_visit_active(|activation, frame, line| {
            frame.admit_retained(&self.program, activation, line, &mut admission)
        })?;
        admission.finish_presentation_custody()?;
        Ok(admission)
    }
}
