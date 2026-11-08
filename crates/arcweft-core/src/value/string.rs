//! Pure read-only String operations shared by admission and execution.

use serde::{Deserialize, Serialize};

use super::{RuntimeEvalError, RuntimeValue};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
/// Read-only text operations; scalar counts deliberately differ from display clusters.
pub enum RuntimeStringOperation {
    Trim,
    ToString,
    ScalarCount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Primitive result contract of an admitted String operation.
pub enum RuntimeStringResultKind {
    String,
    USize,
}

impl RuntimeStringOperation {
    pub const ALL: [Self; 3] = [Self::Trim, Self::ToString, Self::ScalarCount];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Trim => "trim",
            Self::ToString => "to_string",
            Self::ScalarCount => "scalar_count",
        }
    }

    pub fn from_source_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.source_name() == name)
    }

    pub const fn result_kind(self) -> RuntimeStringResultKind {
        match self {
            Self::Trim | Self::ToString => RuntimeStringResultKind::String,
            Self::ScalarCount => RuntimeStringResultKind::USize,
        }
    }

    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Trim => 0,
            Self::ToString => 1,
            Self::ScalarCount => 2,
        }
    }

    pub const fn intrinsic_label(self) -> &'static str {
        match self {
            Self::Trim => "string.trim",
            Self::ToString => "string.to_string",
            Self::ScalarCount => "string.scalar_count",
        }
    }

    pub(crate) const fn intrinsic_tag(self) -> u8 {
        match self {
            Self::Trim => 18,
            Self::ToString => 19,
            Self::ScalarCount => 94,
        }
    }

    pub(crate) const fn from_intrinsic_tag(tag: u8) -> Option<Self> {
        match tag {
            18 => Some(Self::Trim),
            19 => Some(Self::ToString),
            94 => Some(Self::ScalarCount),
            _ => None,
        }
    }

    pub(crate) fn evaluate(self, value: &str) -> Result<RuntimeValue, RuntimeEvalError> {
        Ok(match self {
            Self::Trim => RuntimeValue::String(value.trim().to_owned()),
            Self::ToString => RuntimeValue::String(value.to_owned()),
            Self::ScalarCount => {
                RuntimeValue::usize(u64::try_from(value.chars().count()).map_err(|_| {
                    RuntimeEvalError::UnsupportedPure {
                        name: self.intrinsic_label().to_owned(),
                        reason: "String scalar count exceeds usize wire range".to_owned(),
                    }
                })?)
            }
        })
    }
}
