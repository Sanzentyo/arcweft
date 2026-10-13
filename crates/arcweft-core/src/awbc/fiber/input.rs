//! Borrowed original frame authority for retained-input admission.

use super::{
    AwbcFunctionId, AwbcProgram, FiberFormatAttemptState, FiberFormatState, FiberStateError,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::{RuntimePlaceStorage, RuntimeValueView};

pub(super) struct FrameValueContext<'a, V> {
    pub(super) program: &'a AwbcProgram,
    pub(super) function: AwbcFunctionId,
    pub(super) layout: &'a crate::awbc::schema::AwbcFrameLayout,
    pub(super) binding: Option<&'a RuntimeFunctionEffectInstantiation>,
    pub(super) registers: &'a [RuntimePlaceStorage<V>],
    pub(super) scope_count: usize,
}

impl<V> FrameValueContext<'_, V> {
    pub(super) fn validate_formats(
        &self,
        format: Option<&FiberFormatState<V>>,
        attempts: &[FiberFormatAttemptState<V>],
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .collection(attempts.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if let Some(format) = format {
            format.validate_inputs(self, resources, view)?;
        }
        for attempt in attempts {
            attempt.validate_inputs(self, resources, view)?;
        }
        Ok(())
    }
}
