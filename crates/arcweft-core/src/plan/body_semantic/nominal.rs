//! Actual admitted nominal-domain rows for executable tables two and three.
//! Field/case identities are declaration ordinals scoped by the accepted owner;
//! diagnostic names never replace that identity or the occurrence codec roles.

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::{RuntimeNominalRecordDomain, RuntimePlanTypeProjection, RuntimeVariantDomain};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

impl RuntimeBodySemanticContext<'_> {
    fn write_nominal_domain_identity(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        owner: RuntimePlanTypeId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self.plan.type_table().get(owner).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::UnknownType { ty: owner }
        })?;
        let (
            RuntimePlanTypeProjection::Nominal {
                nominal, layout, ..
            },
            Some(declaration),
        ) = (row.projection(), row.nominal_declaration())
        else {
            encoder.reject_owner();
            return Err(RuntimeBodySemanticError::InvalidNominalDomainOwner { owner });
        };
        encoder.digest(row.semantic_identity().as_bytes());
        encoder.digest(declaration.as_bytes());
        encoder.string(nominal.as_str());
        encoder.digest(layout.as_bytes());
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_type_coordinate(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        if self.plan.type_table().get(ty).is_none() {
            encoder.reject_owner();
            return Err(RuntimeBodySemanticError::UnknownType { ty });
        }
        encoder.ordinal(ty.get().get() - 1);
        encoder.status().map_err(Into::into)
    }
}

impl RuntimeNominalRecordDomain {
    pub(crate) fn executable_semantic_row_digest(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        if !context
            .plan
            .nominal_record_domains()
            .get(self.owner())
            .is_some_and(|row| std::ptr::eq(row, self))
        {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::ForeignNominalDomain);
        }
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(2);
        encoder.tag(self.shape().semantic_tag());
        context.write_nominal_domain_identity(&mut encoder, self.owner())?;
        encoder.count(self.fields().len());
        for (ordinal, field) in self.fields().iter().enumerate() {
            encoder.enter_element();
            encoder.status()?;
            encoder.count(ordinal);
            encoder.ordinal(field.field().zero_based());
            context.write_type_coordinate(&mut encoder, field.ty())?;
        }
        encoder.tag(u8::from(self.data_codec().is_some()));
        if let Some(codec) = self.data_codec() {
            codec.encode_executable_policy(context, &mut encoder)?;
        }
        encoder.finish().map_err(Into::into)
    }
}

impl RuntimeVariantDomain {
    pub(crate) fn executable_semantic_row_digest(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        if !context
            .plan
            .variant_domains()
            .get(self.owner())
            .is_some_and(|row| std::ptr::eq(row, self))
        {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::ForeignNominalDomain);
        }
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(3);
        encoder.tag(0); // the closed admitted variant-domain row kind
        context.write_nominal_domain_identity(&mut encoder, self.owner())?;
        encoder.count(self.cases().len());
        for (ordinal, case) in self.cases().iter().enumerate() {
            encoder.enter_element();
            encoder.status()?;
            encoder.count(ordinal); // source-order role
            encoder.count(ordinal); // accepted owner-scoped case identity
            encoder.tag(u8::from(case.payload().is_some()));
            if let Some(payload) = case.payload() {
                context.write_type_coordinate(&mut encoder, payload)?;
            }
        }
        encoder.tag(u8::from(self.data_codec().is_some()));
        if let Some(codec) = self.data_codec() {
            codec.encode_executable_policy(context, &mut encoder)?;
        }
        encoder.finish().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
