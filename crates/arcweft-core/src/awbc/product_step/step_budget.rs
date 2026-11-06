//! One host step's permission to replenish each existing VM quantum once.
//! These requests borrow no runtime custody and never enter a save/checkpoint.

use super::{ProductDialoguePhase, dialogue::ProductDialogueStore};
use crate::awbc::fiber::{FiberState, FiberStateError, FiberStatus, FiberSuspensionReason};
use crate::awbc::schema::AwbcProgram;
use crate::runtime_id::DialogueActivationId;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct ProductStepBudgets {
    pub(super) main: bool,
    activations: BTreeSet<DialogueActivationId>,
}

impl ProductStepBudgets {
    pub(super) fn for_step(dialogues: &ProductDialogueStore) -> Self {
        Self {
            main: false,
            activations: dialogues
                .active_frames()
                .filter_map(|frame| {
                    matches!(&frame.phase, ProductDialoguePhase::Activating { .. })
                        .then_some(frame.activation)
                })
                .collect(),
        }
    }

    /// Called inside the activation's original complete execution checkpoint.
    pub(super) fn replenish_activation(
        &mut self,
        activation: &DialogueActivationId,
        fiber: &mut FiberState,
        program: &AwbcProgram,
    ) -> Result<(), FiberStateError> {
        if !self.activations.contains(activation) {
            return Ok(());
        }
        if matches!(
            fiber
                .suspension
                .as_ref()
                .map(|suspension| &suspension.reason),
            Some(FiberSuspensionReason::BudgetYield)
        ) {
            fiber.resume_budget_yield(program)?;
        }
        if fiber.status == FiberStatus::Running {
            fiber.replenish_budget();
        }
        self.activations.remove(activation);
        Ok(())
    }
}
