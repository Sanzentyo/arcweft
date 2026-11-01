//! Typed runtime `Option` recognition, construction, and intrinsics.

use super::{RuntimeEvalError, RuntimeValue, runtime_value_label};
use crate::pattern::RuntimeBuiltinVariantCaseIdentity;

impl super::RuntimeRetainedValueAdmission {
    /// Admits the canonical future Option without allocating or copying its
    /// payload. The borrowed payload keeps its original live/inert provenance.
    pub(crate) fn validate_owned_option_preview(
        &mut self,
        payload: Option<super::RuntimeBorrowedValue<'_>>,
        expected: crate::pattern::RuntimeSemanticTypeId,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), super::AwbcRuntimeValueSnapshotError> {
        use super::view::{RuntimeValueSource, RuntimeVariantView};
        use super::{RuntimeBorrowedValue, RuntimeTupleView, RuntimeValueView};
        use crate::entry::schema::value_encoding::ValueAdmissionOrigin;
        use crate::pattern::RuntimeVariantIdentity;

        let case = if payload.is_some() {
            RuntimeBuiltinVariantCaseIdentity::OptionSome
        } else {
            RuntimeBuiltinVariantCaseIdentity::OptionNone
        };
        let owner = RuntimeVariantIdentity::Builtin(case.owner());
        let (ordinal, schema) = case
            .owner()
            .resolve_case(case)
            .expect("canonical Option case belongs to its builtin owner");
        let fields = payload.map(|payload| [payload.value]);
        let tuple = fields
            .as_ref()
            .map(|fields| RuntimeValueView::Tuple(RuntimeTupleView::Views(fields)));
        let origin = payload.map_or(ValueAdmissionOrigin::Live, |payload| payload.origin);
        let value = RuntimeValueView::Variant(RuntimeVariantView::borrowed(
            &owner,
            ordinal,
            schema.name(),
            tuple
                .as_ref()
                .map(|value| RuntimeValueSource::Borrowed { value, origin }),
        ));
        self.validate_owned_typed_borrowed_in_invocation(
            RuntimeBorrowedValue { value, origin },
            expected,
            binding,
        )
    }
}

impl RuntimeValue {
    /// Materializes the canonical runtime representation of `Option::Some`.
    ///
    /// # Panics
    ///
    /// Panics only if the core-owned Option case schema is inconsistent with
    /// its statically defined payload cardinality.
    #[must_use]
    pub fn option_some(value: RuntimeValue) -> Self {
        Self::try_builtin_variant(RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))
            .expect("Option::Some builtin schema requires exactly one payload")
    }

    /// Materializes the canonical runtime representation of `Option::None`.
    ///
    /// # Panics
    ///
    /// Panics only if the core-owned Option case schema is inconsistent with
    /// its statically defined payload cardinality.
    #[must_use]
    pub fn option_none() -> Self {
        Self::try_builtin_variant(RuntimeBuiltinVariantCaseIdentity::OptionNone, None)
            .expect("Option::None builtin schema forbids a payload")
    }

    /// Produces the `None` value for a well-formed runtime `Option` while
    /// preserving the closed Option owner identity.
    #[must_use]
    pub fn option_none_with_same_owner(&self) -> Option<Self> {
        runtime_option_payload(self)?;
        Some(Self::option_none())
    }
}

pub fn evaluate_core_option_is_some_intrinsic(
    value: &RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match runtime_option_payload(value) {
        Some(RuntimeOptionPayload::Some) => Ok(RuntimeValue::Bool(true)),
        Some(RuntimeOptionPayload::None) => Ok(RuntimeValue::Bool(false)),
        None => Err(RuntimeEvalError::ExpectedBracketSeq(format!(
            "core.option.is_some expected Option, found {}",
            runtime_value_label(value)
        ))),
    }
}

pub fn evaluate_core_option_unwrap_intrinsic(
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match value.try_into_builtin_variant_case() {
        Ok((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(payload))) => Ok(payload),
        Ok((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => {
            Err(crate::value::RuntimeExpressionFailure::OptionUnwrapNone.into())
        }
        Ok(_) => Err(RuntimeEvalError::ExpectedBracketSeq(
            "core.option.unwrap expected Option".to_owned(),
        )),
        Err(value) => Err(RuntimeEvalError::ExpectedBracketSeq(format!(
            "core.option.unwrap expected Option, found {}",
            runtime_value_label(&value)
        ))),
    }
}

enum RuntimeOptionPayload {
    Some,
    None,
}

fn runtime_option_payload(value: &RuntimeValue) -> Option<RuntimeOptionPayload> {
    match value.builtin_variant_case().map(|(case, _)| case) {
        Some(RuntimeBuiltinVariantCaseIdentity::OptionSome) => Some(RuntimeOptionPayload::Some),
        Some(RuntimeBuiltinVariantCaseIdentity::OptionNone) => Some(RuntimeOptionPayload::None),
        _ => None,
    }
}
