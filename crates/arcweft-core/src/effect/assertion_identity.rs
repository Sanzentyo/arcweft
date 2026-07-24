//! Persistable assertion identity data shared by runtime transports.

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Artifact-stable identity for one emitted runtime assertion condition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeAssertionGuardId([u8; 16]);

impl RuntimeAssertionGuardId {
    /// Constructs a guard from its fixed-width persisted representation.
    pub fn try_from_bytes(bytes: [u8; 16]) -> Result<Self, RuntimeIdentityDecodeError> {
        if bytes == [0; 16] {
            return Err(RuntimeIdentityDecodeError::ZeroAssertionGuard);
        }
        Ok(Self(bytes))
    }

    /// Returns the exact fixed-width persisted representation.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RuntimeAssertionGuardId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <[u8; 16]>::deserialize(deserializer)?;
        Self::try_from_bytes(bytes).map_err(serde::de::Error::custom)
    }
}

/// Fingerprint of the exact persisted runtime-plan artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeArtifactFingerprint([u8; 32]);

impl RuntimeArtifactFingerprint {
    /// Constructs a fingerprint from the canonical runtime-plan digest bytes.
    pub fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, RuntimeIdentityDecodeError> {
        if bytes == [0; 32] {
            return Err(RuntimeIdentityDecodeError::ZeroArtifactFingerprint);
        }
        Ok(Self(bytes))
    }

    /// Returns the exact canonical runtime-plan digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RuntimeArtifactFingerprint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <[u8; 32]>::deserialize(deserializer)?;
        Self::try_from_bytes(bytes).map_err(serde::de::Error::custom)
    }
}

/// Invalid fixed-width assertion identity data.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeIdentityDecodeError {
    /// The all-zero guard is reserved as an invalid sentinel.
    #[error("runtime assertion guard must not be all zero")]
    ZeroAssertionGuard,
    /// The all-zero artifact fingerprint is reserved as an invalid sentinel.
    #[error("runtime artifact fingerprint must not be all zero")]
    ZeroArtifactFingerprint,
}

#[cfg(test)]
mod tests {
    use super::{RuntimeArtifactFingerprint, RuntimeAssertionGuardId, RuntimeIdentityDecodeError};

    #[test]
    fn checked_identity_constructors_reject_reserved_zero_values() {
        assert_eq!(
            RuntimeAssertionGuardId::try_from_bytes([0; 16]),
            Err(RuntimeIdentityDecodeError::ZeroAssertionGuard)
        );
        assert_eq!(
            RuntimeArtifactFingerprint::try_from_bytes([0; 32]),
            Err(RuntimeIdentityDecodeError::ZeroArtifactFingerprint)
        );
    }

    #[test]
    fn identity_serde_round_trips_fixed_bytes_and_revalidates_decode() {
        let guard = RuntimeAssertionGuardId::try_from_bytes([7; 16]).unwrap();
        let fingerprint = RuntimeArtifactFingerprint::try_from_bytes([9; 32]).unwrap();

        let guard_json = serde_json::to_string(&guard).unwrap();
        let fingerprint_json = serde_json::to_string(&fingerprint).unwrap();
        assert_eq!(
            serde_json::from_str::<RuntimeAssertionGuardId>(&guard_json).unwrap(),
            guard
        );
        assert_eq!(
            serde_json::from_str::<RuntimeArtifactFingerprint>(&fingerprint_json).unwrap(),
            fingerprint
        );

        let zero_guard_json = serde_json::to_string(&[0_u8; 16]).unwrap();
        let zero_fingerprint_json = serde_json::to_string(&[0_u8; 32]).unwrap();
        assert!(serde_json::from_str::<RuntimeAssertionGuardId>(&zero_guard_json).is_err());
        assert!(
            serde_json::from_str::<RuntimeArtifactFingerprint>(&zero_fingerprint_json).is_err()
        );
    }
}
