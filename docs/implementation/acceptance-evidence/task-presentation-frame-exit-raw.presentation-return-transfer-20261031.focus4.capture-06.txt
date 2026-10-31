//! Actual execution authority borrowed for one VM slice, never a saved owner.

use super::{
    AwbcProgram, AwbcRegisterId, FiberState, FiberStateError, VmError, VmExecutionContext,
};
use crate::presentation::resources::{PresentationBanks, PresentationBody};
use crate::value::RuntimePayload;

pub(super) struct VmExecutionResources<'execution> {
    pub(super) context: Option<&'execution VmExecutionContext>,
    pub(super) presentation:
        Option<&'execution mut PresentationBanks<PresentationBody<RuntimePayload>>>,
}

impl VmExecutionResources<'_> {
    /// Admit every actual result resource while it still lives in its source
    /// register. A nested return receives it in the caller's current scope;
    /// a terminal result owns its detached original-bank lease.
    pub(super) fn exit_return_scope(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        registers: impl IntoIterator<Item = AwbcRegisterId>,
    ) -> Result<(), VmError> {
        fiber.validate_cleanup_inputs(program)?;
        let mut escaping = Vec::new();
        for register in registers {
            for handle in fiber
                .active_frame()?
                .register(register)?
                .view()
                .affine_handles()
                .map_err(|error| VmError::Runtime(error.to_string()))?
            {
                if let crate::value::ownership::RuntimeAffineHandle::Presentation {
                    token, ..
                } = handle
                {
                    escaping.push(token);
                }
            }
        }
        let (targets, destination) = fiber
            .presentation_return_scopes_mut()
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if targets.is_empty() && escaping.is_empty() {
            return Ok(());
        }
        let banks = self.presentation.as_deref_mut().ok_or_else(|| {
            FiberStateError::Snapshot(
                crate::presentation::RuntimePresentationError::MissingLedger.into(),
            )
        })?;
        let owner = self
            .context
            .ok_or(VmError::MissingExecutionContext)?
            .program_owner(program)?;
        banks
            .exit_scope_contour(&owner, targets, destination, &escaping)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        Ok(())
    }

    /// All cleanup declarations and actual scope grants precede body movement.
    /// Without a bank, the standalone VM can unwind resource-free frames only.
    pub(super) fn retire_unwind(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
    ) -> Result<(), VmError> {
        fiber.validate_cleanup_inputs(program)?;
        let targets = fiber
            .presentation_unwind_scopes_mut()
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if targets.is_empty() {
            return Ok(());
        }
        let banks = self.presentation.as_deref_mut().ok_or_else(|| {
            FiberStateError::Snapshot(
                crate::presentation::RuntimePresentationError::MissingLedger.into(),
            )
        })?;
        let owner = self
            .context
            .ok_or(VmError::MissingExecutionContext)?
            .program_owner(program)?;
        banks
            .retire_scope_contour(&owner, targets)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        Ok(())
    }
}
