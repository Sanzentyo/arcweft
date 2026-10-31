//! Execution topology supplies bank authority from actual physical owners.

use super::snapshot::{
    AwbcProductChildFiberOwnerSnapshot, AwbcProductExecutorSaveSnapshot,
    AwbcProductExecutorSnapshot, AwbcProductLineTaskWorkInstanceSnapshot,
};
use super::*;
use crate::presentation::resources::{
    PresentationBanks, PresentationBody, PresentationExecutionAdmission, PresentationExecutionRole,
};
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};

impl AwbcProductProgram {
    pub(super) fn preflight_fiber_unwinds<'fiber>(
        &self,
        presentation: Option<&PresentationBanks<PresentationBody<RuntimePayload>>>,
        fibers: impl IntoIterator<Item = &'fiber FiberState>,
    ) -> Result<(), crate::awbc::fiber::FiberStateError> {
        let mut targets = Vec::new();
        for fiber in fibers {
            if matches!(fiber.status, FiberStatus::Running | FiberStatus::Suspended) {
                fiber.validate_cleanup_inputs(self)?;
                targets.extend(fiber.presentation_unwind_scopes().map_err(|error| {
                    crate::awbc::fiber::FiberStateError::Snapshot(error.into())
                })?);
            }
        }
        let Some(banks) = presentation else {
            return if targets.is_empty() {
                Ok(())
            } else {
                Err(crate::awbc::fiber::FiberStateError::Snapshot(
                    crate::presentation::RuntimePresentationError::MissingLedger.into(),
                ))
            };
        };
        banks
            .preflight_scope_contour(
                &crate::task::RuntimeProgramOwner::Awbc(self.program_arc()),
                targets,
            )
            .map_err(|error| crate::awbc::fiber::FiberStateError::Snapshot(error.into()))
    }

    pub(super) fn retire_fiber_scopes<'fiber>(
        &self,
        presentation: &mut Option<PresentationBanks<PresentationBody<RuntimePayload>>>,
        fibers: impl IntoIterator<Item = &'fiber mut FiberState>,
    ) -> Result<(), crate::awbc::fiber::FiberStateError> {
        let mut targets = Vec::new();
        for fiber in fibers {
            if matches!(fiber.status, FiberStatus::Running | FiberStatus::Suspended) {
                fiber.validate_cleanup_inputs(self)?;
                targets.extend(fiber.presentation_unwind_scopes_mut().map_err(|error| {
                    crate::awbc::fiber::FiberStateError::Snapshot(error.into())
                })?);
            }
        }
        let Some(banks) = presentation else {
            return if targets.is_empty() {
                Ok(())
            } else {
                Err(crate::awbc::fiber::FiberStateError::Snapshot(
                    crate::presentation::RuntimePresentationError::MissingLedger.into(),
                ))
            };
        };
        banks
            .retire_scope_contour(
                &crate::task::RuntimeProgramOwner::Awbc(self.program_arc()),
                targets,
            )
            .map_err(|error| crate::awbc::fiber::FiberStateError::Snapshot(error.into()))
    }

    pub(super) fn cancel_fiber(
        &self,
        fiber: &mut FiberState,
        presentation: &mut Option<PresentationBanks<PresentationBody<RuntimePayload>>>,
    ) -> Result<crate::awbc::vm::VmStepOutput, crate::awbc::vm::VmError> {
        self.retire_fiber_scopes(presentation, std::iter::once(&mut *fiber))?;
        crate::awbc::vm::cancel_fiber(self, fiber)
    }
}

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
    pub(super) fn admit_presentation_execution_in(
        &self,
        dialogues: &crate::line_task::RuntimeDialogueRetainedContour<
            '_,
            ActiveDialogue,
            AwbcTypeId,
        >,
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
        dialogues.try_visit_active(
            |activation, frame, _| -> Result<(), AwbcRuntimeValueSnapshotError> {
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
