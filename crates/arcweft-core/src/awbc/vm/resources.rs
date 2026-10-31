//! Actual execution authority borrowed for one VM slice, never a saved owner.

use super::{AwbcProgram, FiberState, FiberStateError, VmError, VmExecutionContext};
use crate::presentation::resources::{PresentationBanks, PresentationBody};
use crate::value::RuntimePayload;

pub(super) struct VmExecutionResources<'execution> {
    pub(super) context: Option<&'execution VmExecutionContext>,
    pub(super) presentation:
        Option<&'execution mut PresentationBanks<PresentationBody<RuntimePayload>>>,
}

impl VmExecutionResources<'_> {
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
