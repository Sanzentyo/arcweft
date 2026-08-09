//! Typed runtime `Option` recognition, construction, and intrinsics.

use super::{RuntimeEvalError, RuntimeValue, runtime_value_label};
use crate::pattern::RuntimeVariantIdentity;

impl RuntimeValue {
    /// Materializes the canonical runtime representation of `Option::Some`.
    #[must_use]
    pub fn option_some(value: RuntimeValue) -> Self {
        Self::Variant {
            owner: RuntimeVariantIdentity::Option,
            ordinal: 0,
            name: "Some".to_owned(),
            payload: Some(Box::new(value)),
        }
    }

    /// Materializes the canonical runtime representation of `Option::None`.
    #[must_use]
    pub fn option_none() -> Self {
        Self::Variant {
            owner: RuntimeVariantIdentity::Option,
            ordinal: 1,
            name: "None".to_owned(),
            payload: None,
        }
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
    match value {
        RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Option,
            ordinal: 0,
            name,
            payload: Some(payload),
        } if name == "Some" => Ok(*payload),
        RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Option,
            ordinal: 1,
            name,
            payload: None,
        } if name == "None" => Err(RuntimeEvalError::ExpectedBracketSeq(
            "core.option.unwrap called on None".to_owned(),
        )),
        value => Err(RuntimeEvalError::ExpectedBracketSeq(format!(
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
    let RuntimeValue::Variant {
        owner,
        ordinal,
        name,
        payload,
    } = value
    else {
        return None;
    };
    if *owner != RuntimeVariantIdentity::Option {
        return None;
    }
    match (*ordinal, name.as_str(), payload.as_deref()) {
        (0, "Some", Some(_)) => Some(RuntimeOptionPayload::Some),
        (1, "None", None) => Some(RuntimeOptionPayload::None),
        _ => None,
    }
}
