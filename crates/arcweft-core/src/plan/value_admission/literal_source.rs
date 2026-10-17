//! Borrowed executable literal admission before any inert source is mapped
//! into live values. The original type/domain tables and parent allowance are
//! the authority; no finished program lease is fabricated for decoding.

use super::{
    Expected, PlanValueAuthority, PlanValueValidation, PlanValueValidationMode,
    RuntimePlanValueAdmissionError,
};
use crate::entry::schema::value_encoding::{self, ValueAdmissionOrigin, ValueResourceAdmission};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeLiteralSourceError,
    RuntimeLiteralSourceLimits, RuntimeValue,
};

impl crate::plan::RuntimePlanInventory {
    pub(crate) fn decode_literal_source_value(
        &self,
        ty: RuntimePlanTypeId,
        bytes: &[u8],
        limits: RuntimeLiteralSourceLimits,
        resources: &mut ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeValue, RuntimeLiteralSourceError> {
        PlanValueAuthority::Inventory(self)
            .decode_literal_source_value(ty, bytes, limits, resources, meter)
    }

    pub(crate) fn materialize_literal_source_value(
        &self,
        ty: RuntimePlanTypeId,
        source: AwbcRuntimeValueSnapshot,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        PlanValueAuthority::Inventory(self).materialize_literal_source_value(ty, source, resources)
    }
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
    pub(in crate::plan) fn decode_literal_source_value(
        self,
        ty: RuntimePlanTypeId,
        bytes: &[u8],
        limits: RuntimeLiteralSourceLimits,
        resources: &mut ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeValue, RuntimeLiteralSourceError> {
        let source =
            AwbcRuntimeValueSnapshot::preflight_literal_source(bytes, limits, resources, meter)?
                .materialize(meter)?;
        let result = self.materialize_literal_source_value(ty, source, resources);
        if result.is_err() {
            meter.reject_owner();
        }
        result.map_err(RuntimeLiteralSourceError::Value)
    }

    /// Consumes the exact source which passed admission; the private capability
    /// never escapes this call or grants a finished executable lease.
    pub(in crate::plan) fn materialize_literal_source_value(
        self,
        ty: RuntimePlanTypeId,
        source: AwbcRuntimeValueSnapshot,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.admit_literal_source_value(ty, &source, resources)
            .map_err(|error| AwbcRuntimeValueSnapshotError::ProgramType(Box::new(error.into())))?;
        source.materialize_admitted_plan_literal(&RuntimePlanLiteralMaterialization {
            authority: self,
        })
    }
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
        let mut validation = PlanValueValidation::with_work(
            self,
            ty,
            work,
            ValueAdmissionOrigin::InertSnapshot,
            PlanValueValidationMode::LiteralSource,
        )?;
        value_encoding::validate_literal_view(view, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| RuntimePlanValueAdmissionError::Value { ty, source })
    }
}

/// Scoped to one already-admitted owned literal graph. Only the source owner
/// constructs it and the consuming value mapper borrows it; it is not Clone,
/// serialized, or a program/task-key authority.
pub(crate) struct RuntimePlanLiteralMaterialization<'a> {
    authority: PlanValueAuthority<'a>,
}

impl RuntimePlanLiteralMaterialization<'_> {
    pub(crate) fn record(
        &self,
        semantic: crate::pattern::RuntimeSemanticTypeId,
        fields: Vec<RuntimeValue>,
        binding: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> crate::value::RuntimeNominalRecordValue {
        let ty = self
            .authority
            .by_semantic(semantic)
            .expect("admitted literal nominal type");
        let declaration = self.authority.declaration(ty);
        let crate::plan::RuntimePlanTypeProjection::Nominal {
            nominal, layout, ..
        } = declaration.projection()
        else {
            unreachable!("admitted literal nominal projection")
        };
        debug_assert_eq!(
            self.authority
                .record(ty)
                .expect("admitted literal record domain")
                .fields()
                .len(),
            fields.len()
        );
        crate::value::RuntimeNominalRecordValue::new(nominal.clone(), semantic, *layout, fields)
            .with_type_instantiation(binding.map(std::sync::Arc::new))
    }

    pub(crate) fn variant(
        &self,
        semantic: crate::pattern::RuntimeSemanticTypeId,
        ordinal: u32,
        payload: Option<RuntimeValue>,
        binding: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> RuntimeValue {
        let ty = self
            .authority
            .by_semantic(semantic)
            .expect("admitted literal variant type");
        let case = self
            .authority
            .declaration(ty)
            .select_variant_case(ty, self.authority.variant(ty), ordinal)
            .expect("admitted literal variant case");
        // All descendants were checked on the original shared work bank. This
        // final move must not revalidate each subtree with a fresh allowance.
        crate::value::RuntimeVariantValue::try_from_plan_case(
            &case,
            payload,
            binding.map(std::sync::Arc::new),
            |_| Ok::<_, std::convert::Infallible>(()),
        )
        .unwrap_or_else(|never| match never {})
    }
}

#[cfg(test)]
mod tests;
