//! Executable helper/method rows read their actual admitted ABI and body.
//! Accepted definition identities are code leaves. Arena IDs, diagnostic
//! labels, inferred/annotated origin and backend support choices are excluded.

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::{
    RuntimeCallableParameter, RuntimePureHelper, RuntimePureInputType, RuntimePureOutputType,
    RuntimeReceiverMode, RuntimeTraitMethod,
};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

impl RuntimePureInputType {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::I8 => 0,
            Self::I16 => 1,
            Self::I32 => 2,
            Self::I64 => 3,
            Self::I128 => 4,
            Self::ISize => 5,
            Self::U8 => 6,
            Self::U16 => 7,
            Self::U32 => 8,
            Self::U64 => 9,
            Self::U128 => 10,
            Self::USize => 11,
            Self::F32 => 12,
            Self::F64 => 13,
            Self::Value => 14,
        }
    }
}

impl RuntimePureOutputType {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::Bool => 0,
            Self::I8 => 1,
            Self::I16 => 2,
            Self::I32 => 3,
            Self::I64 => 4,
            Self::I128 => 5,
            Self::ISize => 6,
            Self::U8 => 7,
            Self::U16 => 8,
            Self::U32 => 9,
            Self::U64 => 10,
            Self::U128 => 11,
            Self::USize => 12,
            Self::F32 => 13,
            Self::F64 => 14,
            Self::Value => 15,
        }
    }
}

impl RuntimeBodySemanticContext<'_> {
    fn write_pure_inputs(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        inputs: &[RuntimeCallableParameter],
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        encoder.count(inputs.len());
        for (ordinal, input) in inputs.iter().enumerate() {
            encoder.enter_element();
            encoder.status()?;
            encoder.count(ordinal);
            encoder.digest(input.identity().as_bytes());
            encoder.tag(input.passing().semantic_tag());
            encoder.tag(input.abi().semantic_tag());
            self.write_local(encoder, input.local())?;
        }
        encoder.status().map_err(Into::into)
    }
}

impl RuntimePureHelper {
    pub(crate) fn executable_semantic_row_digest(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        if !context
            .plan
            .pure_helpers()
            .get(self.id.0)
            .is_some_and(|row| std::ptr::eq(row, self))
        {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::ForeignPureHelperRow);
        }
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(10);
        encoder.tag(0);
        encoder.digest(self.definition.as_bytes());
        context.write_pure_inputs(&mut encoder, &self.inputs)?;
        encoder.tag(self.output_type.semantic_tag());
        context.write_expression(&mut encoder, &self.expr)?;
        encoder.finish().map_err(Into::into)
    }
}

impl RuntimeTraitMethod {
    pub(crate) fn executable_semantic_row_digest(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        if !context
            .plan
            .trait_methods()
            .get(self.id.0)
            .is_some_and(|row| std::ptr::eq(row, self))
        {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::ForeignTraitMethodRow);
        }
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(11);
        encoder.tag(0);
        encoder.digest(self.definition.as_bytes());
        encoder.tag(match self.receiver {
            RuntimeReceiverMode::Owned => 0,
            RuntimeReceiverMode::SharedRef => 1,
            RuntimeReceiverMode::MutRef => 2,
        });
        context.write_pure_inputs(&mut encoder, &self.inputs)?;
        encoder.tag(self.output_type.semantic_tag());
        context.write_expression(&mut encoder, &self.body)?;
        encoder.finish().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
