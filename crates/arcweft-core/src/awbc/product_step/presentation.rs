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

struct ProductPresentationContour<'a, F> {
    program: &'a crate::awbc::schema::AwbcProgram,
    banks: PresentationExecutionAdmission<'a>,
    receivers: dialogue::ProductDialogueReceivers<'a, F>,
}

impl<'a, V, O, N, B, R, G> ProductPresentationContour<'a, FiberState<V, O, N, B, R, G>> {
    fn new(
        program: &'a crate::awbc::schema::AwbcProgram,
        banks: PresentationExecutionAdmission<'a>,
    ) -> Self {
        Self {
            program,
            banks,
            receivers: dialogue::ProductDialogueReceivers::default(),
        }
    }

    fn register_fiber(
        &mut self,
        fiber: &'a FiberState<V, O, N, B, R, G>,
        role: PresentationExecutionRole,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.banks
            .register_fiber(fiber.instance, fiber.presentation_bank(), role, admission)?;
        let receiver = match role {
            PresentationExecutionRole::Dialogue(parent) => {
                dialogue::ProductDialogueReceiver::LineChild {
                    instance: fiber.instance,
                    parent,
                }
            }
            _ => dialogue::ProductDialogueReceiver::Fiber(fiber.instance),
        };
        Ok(self.receivers.register_fiber(fiber, receiver)?)
    }

    fn register_dialogue<E, L, P, H>(
        &mut self,
        activation: DialogueActivationId,
        frame: &'a AwbcProductActiveDialogue<
            V,
            O,
            E,
            AwbcProductDialoguePhase<FiberState<V, O, N, B, R, G>, L, P>,
            H,
        >,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.banks.admit_dialogue(activation, admission)?;
        if let Some(fiber) = frame.activation_fiber() {
            self.banks.register_fiber(
                fiber.instance,
                fiber.presentation_bank(),
                PresentationExecutionRole::Dialogue(activation),
                admission,
            )?;
            self.receivers.register_fiber(
                fiber,
                dialogue::ProductDialogueReceiver::Activation(activation),
            )?;
        }
        // The bank/allocator and the original activation root authenticate
        // the frame before any suspension on that frame can supply an edge.
        frame
            .original_content_group(self.program)
            .map_err(|error| AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            })?;
        self.receivers.register_dialogue(activation, frame)?;
        Ok(())
    }

    fn finish(
        self,
        banks: impl Iterator<Item = crate::runtime_id::RuntimePresentationBankId>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<
        dialogue::ProductDialogueReceivers<'a, FiberState<V, O, N, B, R, G>>,
        AwbcRuntimeValueSnapshotError,
    > {
        self.receivers.validate(self.program)?;
        self.banks.finish(banks, admission)?;
        Ok(self.receivers)
    }
}

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
        if !fiber.has_exit_work() {
            self.retire_fiber_scopes(presentation, std::iter::once(&mut *fiber))?;
        }
        crate::awbc::vm::cancel_fiber(self, fiber)
    }
}

impl AwbcProductProgram {
    pub(super) fn trap_fiber(
        &self,
        fiber: &mut FiberState,
        presentation: &mut Option<PresentationBanks<PresentationBody<RuntimePayload>>>,
        trap: crate::awbc::fiber::FiberTrap,
    ) -> Result<crate::awbc::vm::VmStepOutput, crate::awbc::vm::VmError> {
        if !fiber.has_exit_work() {
            self.retire_fiber_scopes(presentation, std::iter::once(&mut *fiber))?;
        }
        crate::awbc::vm::trap_fiber(self, fiber, trap)
    }
}

impl ProductChildFiberOwner {
    pub(super) fn presentation_role(&self) -> PresentationExecutionRole {
        match self {
            Self::Independent => PresentationExecutionRole::Independent,
            Self::Program { parent, .. } => {
                PresentationExecutionRole::AttachedChild { parent: *parent }
            }
            Self::FlowDeferred { owner, .. } => PresentationExecutionRole::AttachedChild {
                parent: owner.fiber,
            },
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
                PresentationExecutionRole::AttachedChild { parent: *parent }
            }
            Self::FlowDeferred { owner, .. } => PresentationExecutionRole::AttachedChild {
                parent: owner.fiber,
            },
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
        let mut context = ProductPresentationContour::new(
            &self.program,
            PresentationExecutionAdmission::new(
                self.next_fiber_instance.last_issued(),
                self.dialogue_occurrences
                    .iter()
                    .map(|(&(owner, content), next)| (owner, content, next)),
                admission,
            )?,
        );
        context.register_fiber(&self.fiber, PresentationExecutionRole::Root, admission)?;
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
            context.register_fiber(&child.fiber, role, admission)?;
        }
        dialogues.try_visit_active(|activation, frame, _| {
            context.register_dialogue(*activation, frame, admission)
        })?;
        context
            .finish(
                self.presentation.iter().flat_map(|banks| banks.bank_ids()),
                admission,
            )
            .map(|_| ())
    }
}

impl AwbcProductExecutorSnapshot {
    pub(super) fn admit_presentation_execution<'a>(
        &'a self,
        program: &'a crate::awbc::schema::AwbcProgram,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<
        dialogue::ProductDialogueReceivers<'a, crate::awbc::fiber::AwbcFiberStateSnapshot>,
        AwbcProductStepBuildError,
    > {
        let invalid =
            |error: AwbcRuntimeValueSnapshotError| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            };
        let mut context = ProductPresentationContour::new(
            program,
            PresentationExecutionAdmission::new(
                self.next_fiber_instance.last_issued(),
                self.dialogue_occurrences
                    .iter()
                    .map(|row| (row.owner, row.content, &row.next)),
                admission,
            )
            .map_err(invalid)?,
        );
        context
            .register_fiber(&self.fiber, PresentationExecutionRole::Root, admission)
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
                .register_fiber(&child.fiber, role, admission)
                .map_err(invalid)?;
        }
        self.dialogues
            .try_visit_active(|activation, frame, _| {
                context.register_dialogue(*activation, frame, admission)
            })
            .map_err(invalid)?;
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
        program: &crate::awbc::schema::AwbcProgram,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcProductStepBuildError> {
        let invalid =
            |error: AwbcRuntimeValueSnapshotError| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            };
        let mut context = ProductPresentationContour::new(
            program,
            PresentationExecutionAdmission::new(
                self.next_fiber_instance.last_issued(),
                self.dialogue_occurrences
                    .iter()
                    .map(|row| (row.owner, row.content, &row.next)),
                admission,
            )
            .map_err(invalid)?,
        );
        context
            .register_fiber(&self.fiber, PresentationExecutionRole::Root, admission)
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
                .register_fiber(&child.fiber, role, admission)
                .map_err(invalid)?;
        }
        self.dialogues
            .try_visit_active(|activation, frame, _| {
                context.register_dialogue(*activation, frame, admission)
            })
            .map_err(invalid)?;
        context
            .finish(
                self.presentation.iter().flat_map(|banks| banks.bank_ids()),
                admission,
            )
            .map(|_| ())
            .map_err(invalid)
    }
}
