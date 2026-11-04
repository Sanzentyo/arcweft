//! Plain prospective roots use the original retained resource allowance.

use super::{
    AwbcRuntimeValueSnapshotError, RetainedTypeObligation, RuntimeFunctionEffectInstantiation,
    RuntimeRetainedValueAdmission, RuntimeSemanticTypeId, RuntimeValue, ValueAdmissionOrigin,
};
use crate::pattern::RuntimeOpaqueTypeOwner;
use crate::value::{
    RuntimeOpaqueValueClass, RuntimeOpaqueView, RuntimePlainValueRecipe, RuntimeValueView,
};

impl RuntimeRetainedValueAdmission {
    /// Admits a prospective scalar/tuple/Option root under its invocation before
    /// allocating owning containers or copying borrowed scalar text.
    pub fn construct_plain_value(
        &mut self,
        recipe: &RuntimePlainValueRecipe<'_>,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> Result<RuntimeValue, Box<AwbcRuntimeValueSnapshotError>> {
        self.validate_owned_typed_borrowed_in_invocation(recipe.borrowed(), expected, binding)?;
        Ok(recipe.materialize())
    }

    /// Constructs one plain opaque with fully resolved original stored source
    /// types. The envelope and complete payload share a single admission walk.
    /// Producer-owned payload grammar must be checked on the borrowed recipe.
    pub fn construct_plain_opaque(
        &mut self,
        owner: &RuntimeOpaqueTypeOwner,
        payload_type: RuntimeSemanticTypeId,
        recipe: &RuntimePlainValueRecipe<'_>,
    ) -> Result<RuntimeValue, Box<AwbcRuntimeValueSnapshotError>> {
        let result: Result<_, Box<AwbcRuntimeValueSnapshotError>> = (|| {
            self.status()?;
            if owner.value_class() != RuntimeOpaqueValueClass::Plain {
                return Err(Box::new(AwbcRuntimeValueSnapshotError::AffineValue));
            }
            let payload = recipe.view();
            self.validate_view(
                RuntimeValueView::Opaque(RuntimeOpaqueView::Construction {
                    owner,
                    payload: &payload,
                }),
                ValueAdmissionOrigin::Live,
                Some(RetainedTypeObligation::OpaquePayload {
                    outer: owner.semantic_identity(),
                    payload: payload_type,
                }),
            )?;
            Ok(owner
                .try_wrap(recipe.materialize())
                .map_err(AwbcRuntimeValueSnapshotError::from)?)
        })();
        result.inspect_err(|error| self.retain_failure((**error).clone()))
    }
}
#[cfg(test)]
mod tests;
