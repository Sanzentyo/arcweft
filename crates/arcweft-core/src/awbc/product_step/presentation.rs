//! Execution topology supplies bank authority from actual physical owners.

use super::snapshot::{
    AwbcProductChildFiberOwnerSnapshot, AwbcProductExecutorSaveSnapshot,
    AwbcProductExecutorSnapshot, AwbcProductLineTaskWorkInstanceSnapshot,
};
use super::*;
use crate::presentation::resources::{PresentationExecutionAdmission, PresentationExecutionRole};
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};

impl ProductChildFiberOwner {
    fn presentation_role(&self) -> PresentationExecutionRole {
        match self {
            Self::Independent => PresentationExecutionRole::Independent,
            Self::Program { parent, .. } => {
                PresentationExecutionRole::ProgramChild { parent: *parent }
            }
            Self::LineTask { tag, .. } => PresentationExecutionRole::Dialogue(*tag.activation_id()),
            Self::Deferred { activation, .. } | Self::ScopedDeferred { activation, .. } => {
                PresentationExecutionRole::Dialogue(*activation)
            }
        }
    }
}

impl AwbcProductChildFiberOwnerSnapshot {
    fn presentation_role(&self) -> PresentationExecutionRole {
        match self {
            Self::Independent => PresentationExecutionRole::Independent,
            Self::Program { parent, .. } => {
                PresentationExecutionRole::ProgramChild { parent: *parent }
            }
            Self::LineTask { tag, .. } => {
                PresentationExecutionRole::Dialogue(match &tag.instance {
                    AwbcProductLineTaskWorkInstanceSnapshot::Activation(activation) => *activation,
                    AwbcProductLineTaskWorkInstanceSnapshot::Scheduled(token) => {
                        *token.activation()
                    }
                })
            }
            Self::Deferred { activation, .. } | Self::ScopedDeferred { activation, .. } => {
                PresentationExecutionRole::Dialogue(*activation)
            }
        }
    }
}

impl AwbcProductStepExecutor {
    pub(super) fn admit_presentation_execution(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let mut context = PresentationExecutionAdmission::new(
            self.next_fiber_instance.last_issued(),
            self.dialogue_occurrences
                .iter()
                .map(|(&(owner, content), next)| (owner, content, next)),
            admission,
        )?;
        context.register_fiber(
            self.fiber.instance,
            self.fiber.presentation_bank(),
            PresentationExecutionRole::Root,
            admission,
        )?;
        admission.validate_metadata_collection(self.child_fibers.len())?;
        for child in &self.child_fibers {
            let role = if matches!(
                child.fiber.root,
                crate::awbc::fiber::AwbcFiberRoot::RootReducer(_)
            ) {
                PresentationExecutionRole::Root
            } else {
                child.owner.presentation_role()
            };
            context.register_fiber(
                child.fiber.instance,
                child.fiber.presentation_bank(),
                role,
                admission,
            )?;
        }
        self.dialogues
            .try_visit_active(|activation, frame| {
                context.admit_dialogue(*activation, admission)?;
                if let Some(fiber) = frame.activation_fiber() {
                    context.register_fiber(
                        fiber.instance,
                        fiber.presentation_bank(),
                        PresentationExecutionRole::Dialogue(*activation),
                        admission,
                    )?;
                }
                Ok(())
            })
            .map_err(
                |error: crate::line_task::RuntimeDialogueRegistrySnapshotError| {
                    AwbcRuntimeValueSnapshotError::Message {
                        message: error.to_string(),
                    }
                },
            )?;
        context.finish(
            self.presentation.iter().flat_map(|banks| banks.bank_ids()),
            admission,
        )
    }
}

impl AwbcProductExecutorSnapshot {
    pub(super) fn admit_presentation_execution(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcProductStepBuildError> {
        let invalid =
            |error: AwbcRuntimeValueSnapshotError| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            };
        let mut context = PresentationExecutionAdmission::new(
            self.next_fiber_instance.last_issued(),
            self.dialogue_occurrences
                .iter()
                .map(|row| (row.owner, row.content, &row.next)),
            admission,
        )
        .map_err(invalid)?;
        context
            .register_fiber(
                self.fiber.instance,
                self.fiber.presentation_bank(),
                PresentationExecutionRole::Root,
                admission,
            )
            .map_err(invalid)?;
        admission
            .validate_metadata_collection(self.child_fibers.len())
            .map_err(invalid)?;
        for child in &self.child_fibers {
            let role = if matches!(
                child.fiber.root,
                crate::awbc::fiber::AwbcFiberRoot::RootReducer(_)
            ) {
                PresentationExecutionRole::Root
            } else {
                child.owner.presentation_role()
            };
            context
                .register_fiber(
                    child.fiber.instance,
                    child.fiber.presentation_bank(),
                    role,
                    admission,
                )
                .map_err(invalid)?;
        }
        self.dialogues
            .try_visit_active(|activation, frame, _| {
                context
                    .admit_dialogue(*activation, admission)
                    .map_err(|error| {
                        crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                            message: error.to_string(),
                        }
                    })?;
                if let Some(fiber) = frame.activation_fiber() {
                    context
                        .register_fiber(
                            fiber.instance,
                            fiber.presentation_bank(),
                            PresentationExecutionRole::Dialogue(*activation),
                            admission,
                        )
                        .map_err(|error| {
                            crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                message: error.to_string(),
                            }
                        })?;
                }
                Ok(())
            })
            .map_err(
                |error: crate::line_task::RuntimeDialogueRegistrySnapshotError| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: error.to_string(),
                    }
                },
            )?;
        context
            .finish(
                self.presentation.iter().flat_map(|banks| banks.bank_ids()),
                admission,
            )
            .map_err(invalid)
    }
}

impl AwbcProductExecutorSaveSnapshot {
    pub(super) fn admit_presentation_execution(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcProductStepBuildError> {
        let invalid =
            |error: AwbcRuntimeValueSnapshotError| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            };
        let mut context = PresentationExecutionAdmission::new(
            self.next_fiber_instance.last_issued(),
            self.dialogue_occurrences
                .iter()
                .map(|row| (row.owner, row.content, &row.next)),
            admission,
        )
        .map_err(invalid)?;
        context
            .register_fiber(
                self.fiber.instance,
                self.fiber.presentation_bank(),
                PresentationExecutionRole::Root,
                admission,
            )
            .map_err(invalid)?;
        admission
            .validate_metadata_collection(self.child_fibers.len())
            .map_err(invalid)?;
        for child in &self.child_fibers {
            let role = if matches!(
                child.fiber.root,
                crate::awbc::fiber::AwbcFiberRoot::RootReducer(_)
            ) {
                PresentationExecutionRole::Root
            } else {
                child.owner.presentation_role()
            };
            context
                .register_fiber(
                    child.fiber.instance,
                    child.fiber.presentation_bank(),
                    role,
                    admission,
                )
                .map_err(invalid)?;
        }
        self.dialogues
            .try_visit_active(|activation, frame, _| {
                context
                    .admit_dialogue(*activation, admission)
                    .map_err(|error| {
                        crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                            message: error.to_string(),
                        }
                    })?;
                if let Some(fiber) = frame.activation_fiber() {
                    context
                        .register_fiber(
                            fiber.instance,
                            fiber.presentation_bank(),
                            PresentationExecutionRole::Dialogue(*activation),
                            admission,
                        )
                        .map_err(|error| {
                            crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                message: error.to_string(),
                            }
                        })?;
                }
                Ok(())
            })
            .map_err(
                |error: crate::line_task::RuntimeDialogueRegistrySnapshotError| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: error.to_string(),
                    }
                },
            )?;
        context
            .finish(
                self.presentation.iter().flat_map(|banks| banks.bank_ids()),
                admission,
            )
            .map_err(invalid)
    }
}
