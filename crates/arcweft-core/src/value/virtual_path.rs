//! Standard virtual file identities. Physical mounts remain host-owned.

use crate::pattern::{RUNTIME_STANDARD_VIRTUAL_PATH, RuntimeOpaqueTypeOwner};
use crate::value::RuntimeValue;
use thiserror::Error;

/// Logical file spaces available to the standard path capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeVirtualPathSpace {
    Save,
    Asset,
    Temp,
    Export,
}

impl RuntimeVirtualPathSpace {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Asset => "asset",
            Self::Temp => "temp",
            Self::Export => "export",
        }
    }

    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "save" => Some(Self::Save),
            "asset" => Some(Self::Asset),
            "temp" => Some(Self::Temp),
            "export" => Some(Self::Export),
            _ => None,
        }
    }
}

/// Producer-checked standard VirtualPath payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeVirtualPath {
    space: RuntimeVirtualPathSpace,
    path: String,
}

impl RuntimeVirtualPath {
    #[must_use]
    pub fn new(space: RuntimeVirtualPathSpace, path: impl Into<String>) -> Self {
        Self {
            space,
            path: path.into(),
        }
    }

    #[must_use]
    pub const fn space(&self) -> RuntimeVirtualPathSpace {
        self.space
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Canonical logical path carried by the file task protocol.
    #[must_use]
    pub fn runtime_label(&self) -> String {
        format!("{}:{}", self.space.as_str(), self.path)
    }

    #[must_use]
    pub fn exact_owner() -> RuntimeOpaqueTypeOwner {
        RUNTIME_STANDARD_VIRTUAL_PATH
            .monomorphic_owner()
            .expect("VirtualPath has no type arguments")
    }

    /// Wraps this payload with the standard type's exact producer evidence.
    #[must_use]
    pub fn into_value(self) -> RuntimeValue {
        Self::exact_owner()
            .try_wrap(RuntimeValue::Tuple(vec![
                RuntimeValue::String(self.space.as_str().to_owned()),
                RuntimeValue::String(self.path),
            ]))
            .expect("the standard VirtualPath owner is exact")
    }
}

/// Invalid standard opaque path received at an execution boundary.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeVirtualPathError {
    #[error("runtime value is not owned by the exact standard VirtualPath type")]
    InvalidOwner,
    #[error("VirtualPath payload must contain a known logical space and a path")]
    InvalidPayload,
}

impl TryFrom<&RuntimeValue> for RuntimeVirtualPath {
    type Error = RuntimeVirtualPathError;

    fn try_from(value: &RuntimeValue) -> Result<Self, Self::Error> {
        let RuntimeValue::Opaque(value) = value else {
            return Err(RuntimeVirtualPathError::InvalidOwner);
        };
        if !Self::exact_owner().accepts_opaque_value(value) {
            return Err(RuntimeVirtualPathError::InvalidOwner);
        }
        let RuntimeValue::Tuple(parts) = value.payload() else {
            return Err(RuntimeVirtualPathError::InvalidPayload);
        };
        let [RuntimeValue::String(space), RuntimeValue::String(path)] = parts.as_slice() else {
            return Err(RuntimeVirtualPathError::InvalidPayload);
        };
        let space = RuntimeVirtualPathSpace::from_label(space)
            .ok_or(RuntimeVirtualPathError::InvalidPayload)?;
        Ok(Self::new(space, path.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId};

    #[test]
    fn virtual_paths_round_trip_with_exact_standard_ownership() {
        for space in [
            RuntimeVirtualPathSpace::Save,
            RuntimeVirtualPathSpace::Asset,
            RuntimeVirtualPathSpace::Temp,
            RuntimeVirtualPathSpace::Export,
        ] {
            let path = RuntimeVirtualPath::new(space, "nested/profile.json");
            assert_eq!(
                RuntimeVirtualPath::try_from(&path.clone().into_value()),
                Ok(path)
            );
        }
    }

    #[test]
    fn virtual_paths_reject_strings_foreign_identity_and_malformed_payload() {
        assert_eq!(
            RuntimeVirtualPath::try_from(&RuntimeValue::String("save:a".to_owned())),
            Err(RuntimeVirtualPathError::InvalidOwner)
        );
        let expected = RuntimeVirtualPath::exact_owner();
        let foreign = RuntimeOpaqueTypeOwner::exact(
            expected.producer().clone(),
            RuntimeSemanticTypeId::from_bytes([0xa5; 32]),
        );
        assert_eq!(
            RuntimeVirtualPath::try_from(&foreign.try_wrap(RuntimeValue::Unit).unwrap()),
            Err(RuntimeVirtualPathError::InvalidOwner)
        );
        assert_eq!(
            RuntimeVirtualPath::try_from(
                &expected
                    .try_wrap(RuntimeValue::String("save:a".to_owned()))
                    .unwrap()
            ),
            Err(RuntimeVirtualPathError::InvalidPayload)
        );
    }
}
