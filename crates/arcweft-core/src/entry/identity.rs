//! Stable entry, callable, command, and digest identities.

use serde::{Deserialize, Serialize};
use thiserror::Error;
macro_rules! digest_type {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            Deserialize,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
            Serialize,
        )]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const ZERO: Self = Self([0; 32]);

            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

digest_type!(EntryBindingIdentity);
digest_type!(TypeLayoutHash);
digest_type!(CallableContractHash);
digest_type!(FlowContractHash);
digest_type!(AgentPolicyHash);
digest_type!(RuntimeValueDigest);

/// Stable nominal identity selected by a checked entry role.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeNominalTypeId(String);

/// Stable semantic identity of an ordinary callable selected by checked
/// lowering.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeCallableId {
    identity: String,
}

/// Stable identity of one opaque command constructor.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeCommandConstructorId(String);

/// Stable adapter-visible target identity carried by an opaque command.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeCommandTargetId(String);

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeIdentityError {
    #[error("{kind} identity cannot be empty")]
    Empty { kind: &'static str },
    #[error("{kind} identity contains a control character at byte {byte}")]
    Control { kind: &'static str, byte: usize },
}

impl RuntimeNominalTypeId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, RuntimeIdentityError> {
        validate_identity("nominal type", value.into()).map(Self)
    }

    /// Projects an accepted semantic digest into the versioned runtime
    /// nominal namespace without retaining a display or source spelling.
    #[must_use]
    pub fn from_checked_digest(digest: [u8; 32]) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut identity = String::with_capacity("arcweft.nominal.checked.v1.".len() + 64);
        identity.push_str("arcweft.nominal.checked.v1.");
        for byte in digest {
            identity.push(char::from(HEX[usize::from(byte >> 4)]));
            identity.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Self(identity)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RuntimeCallableId {
    pub fn try_new(identity: impl Into<String>) -> Result<Self, RuntimeIdentityError> {
        Ok(Self {
            identity: validate_identity("callable", identity.into())?,
        })
    }

    /// Projects a checked semantic digest into the versioned runtime identity
    /// namespace. This projection is intentionally one-way.
    #[must_use]
    pub fn from_checked_digest(digest: [u8; 32]) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut identity = String::with_capacity("arcweft.checked.v1.".len() + 64);
        identity.push_str("arcweft.checked.v1.");
        for byte in digest {
            identity.push(char::from(HEX[usize::from(byte >> 4)]));
            identity.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Self { identity }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identity
    }
}

impl RuntimeCommandConstructorId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, RuntimeIdentityError> {
        validate_identity("command constructor", value.into()).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RuntimeCommandTargetId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, RuntimeIdentityError> {
        validate_identity("command target", value.into()).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_identity(kind: &'static str, value: String) -> Result<String, RuntimeIdentityError> {
    if value.is_empty() {
        return Err(RuntimeIdentityError::Empty { kind });
    }
    if let Some((byte, _)) = value
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(RuntimeIdentityError::Control { kind, byte });
    }
    Ok(value)
}
