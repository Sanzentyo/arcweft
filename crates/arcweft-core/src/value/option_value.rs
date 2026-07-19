//! Typed runtime `Option` recognition, construction, and intrinsics.

use super::{RuntimeEvalError, RuntimeValue, runtime_value_label};

impl RuntimeValue {
    /// Produces the `None` value for a well-formed runtime `Option` while
    /// preserving whether its path was implicit or explicitly `Option`.
    #[must_use]
    pub fn option_none_with_same_path(&self) -> Option<Self> {
        runtime_option_payload(self)?;
        let Self::Variant { path, .. } = self else {
            return None;
        };
        Some(Self::Variant {
            path: path.clone(),
            name: "None".to_owned(),
            payload: None,
        })
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
            path,
            name,
            payload: Some(payload),
        } if is_option_path(path.as_deref()) && name == "Some" => Ok(*payload),
        RuntimeValue::Variant {
            path,
            name,
            payload: None,
        } if is_option_path(path.as_deref()) && name == "None" => Err(
            RuntimeEvalError::ExpectedBracketSeq("core.option.unwrap called on None".to_owned()),
        ),
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
        path,
        name,
        payload,
    } = value
    else {
        return None;
    };
    if !is_option_path(path.as_deref()) {
        return None;
    }
    match (name.as_str(), payload.as_deref()) {
        ("Some", Some(_)) => Some(RuntimeOptionPayload::Some),
        ("None", None) => Some(RuntimeOptionPayload::None),
        _ => None,
    }
}

fn is_option_path(path: Option<&str>) -> bool {
    path.is_none_or(|path| path == "Option")
}

pub(super) fn runtime_option_some(value: RuntimeValue) -> RuntimeValue {
    RuntimeValue::Variant {
        path: Some("Option".to_owned()),
        name: "Some".to_owned(),
        payload: Some(Box::new(value)),
    }
}

pub(super) fn runtime_option_none() -> RuntimeValue {
    RuntimeValue::Variant {
        path: Some("Option".to_owned()),
        name: "None".to_owned(),
        payload: None,
    }
}
