//! Typed runtime `Option` recognition, construction, and intrinsics.

use super::{RuntimeEvalError, RuntimeValue, runtime_value_label};
use crate::pattern::RuntimeBuiltinVariantCaseIdentity;

impl RuntimeValue {
    /// Materializes the canonical runtime representation of `Option::Some`.
    #[must_use]
    pub fn option_some(value: RuntimeValue) -> Self {
        Self::try_builtin_variant(RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))
            .expect("Option::Some builtin schema requires exactly one payload")
    }

    /// Materializes the canonical runtime representation of `Option::None`.
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
    match value.builtin_variant_case().map(|(case, _)| case) {
        Some(RuntimeBuiltinVariantCaseIdentity::OptionSome) => {
            let RuntimeValue::Variant {
                payload: Some(payload),
                ..
            } = value
            else {
                unreachable!("admitted Option::Some has a payload")
            };
            Ok(*payload)
        }
        Some(RuntimeBuiltinVariantCaseIdentity::OptionNone) => Err(
            RuntimeEvalError::ExpectedBracketSeq("core.option.unwrap called on None".to_owned()),
        ),
        _ => Err(RuntimeEvalError::ExpectedBracketSeq(format!(
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
