//! Borrowed executable literal admission before any inert source is mapped
//! into live values. The original type/domain tables and parent allowance are
//! the authority; no finished program lease is fabricated for decoding.

use super::{Expected, PlanValueAuthority, PlanValueValidation, RuntimePlanValueAdmissionError};
use crate::entry::schema::value_encoding::{self, ValueAdmissionOrigin, ValueResourceAdmission};
use crate::runtime_id::RuntimePlanTypeId;
use crate::value::AwbcRuntimeValueSnapshot;

impl crate::plan::RuntimePlanInventory {
    pub(crate) fn admit_literal_source_value(
        &self,
        ty: RuntimePlanTypeId,
        source: &AwbcRuntimeValueSnapshot,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), RuntimePlanValueAdmissionError> {
        PlanValueAuthority::Inventory(self).admit_literal_source_value(ty, source, resources)
    }
}

impl PlanValueAuthority<'_> {
    /// Counts the complete original inert graph, then checks all literal type
    /// obligations using that same sticky work bank. Repeated source rows share
    /// the caller's resource admission; this method grants no fresh allowance.
    pub(in crate::plan) fn admit_literal_source_value(
        self,
        ty: RuntimePlanTypeId,
        source: &AwbcRuntimeValueSnapshot,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), RuntimePlanValueAdmissionError> {
        resources.set_origin(ValueAdmissionOrigin::InertSnapshot);
        let view = source.view();
        resources
            .validate_view(view, 0, None)
            .map_err(|source| RuntimePlanValueAdmissionError::Value { ty, source })?;
        let work = resources.work();
        let limits = work.limits();
        let mut validation =
            PlanValueValidation::with_work(self, ty, work, ValueAdmissionOrigin::InertSnapshot)?;
        value_encoding::validate_literal_view(view, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| RuntimePlanValueAdmissionError::Value { ty, source })
    }
}

#[cfg(test)]
mod tests;
