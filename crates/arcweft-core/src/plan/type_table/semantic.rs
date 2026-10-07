//! Table-zero row digest from the actual admitted type declaration.
//! Whole-image ordering, graph admission and memoization belong to the common
//! private sealer; this owner writes its exact ordered fields on the same meter.

use super::RuntimePlanTypeDeclaration;
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

impl RuntimePlanTypeDeclaration {
    pub(crate) fn executable_semantic_row_digest(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(0); // canonical type table
        encoder.tag(self.projection().executable_semantic_kind());
        encoder.digest(self.semantic_identity().as_bytes());
        encoder.tag(u8::from(self.nominal_declaration().is_some()));
        if let Some(declaration) = self.nominal_declaration() {
            encoder.digest(declaration.as_bytes());
        }
        self.scope().encode_semantic_scope(&mut encoder)?;
        self.projection().encode_executable_shape(&mut encoder)?;
        encoder.tag(u8::from(self.data_codec().is_some()));
        if let Some(codec) = self.data_codec() {
            codec.encode_executable_policy(context, &mut encoder)?;
        }
        encoder.finish().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
