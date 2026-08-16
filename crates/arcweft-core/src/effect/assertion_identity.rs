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
    use super::{RuntimeArtifactFingerprint, RuntimeAssertionGuardId};
    use crate::{
        effect::{
            LineEffectRequest, RuntimeAssertion, RuntimeAssertionFailure, RuntimeAssertionProfile,
            RuntimeEffectExpr, RuntimeEffectMaterializeError,
        },
        runtime_id::RuntimePlanTypeId,
        value::{RuntimeExpr, RuntimeExprKind, RuntimeValue},
    };
    use std::num::NonZeroU32;

    fn value_expr(value: RuntimeValue) -> RuntimeExpr {
        RuntimeExpr::from_admitted_parts(
            RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
            RuntimeExprKind::Value(value),
        )
    }

    fn failure_fixture() -> RuntimeAssertionFailure {
        RuntimeAssertionFailure::new(RuntimeAssertion::new(
            RuntimeAssertionGuardId::try_from_bytes([7; 16]).unwrap(),
            "ready".to_owned(),
            "must be ready".to_owned(),
            RuntimeAssertionProfile::Always,
        ))
    }

    fn assert_failure_payload(decoded: &RuntimeAssertionFailure) {
        assert_eq!(
            decoded.assertion().guard(),
            RuntimeAssertionGuardId::try_from_bytes([7; 16]).unwrap()
        );
        assert_eq!(decoded.assertion().condition(), "ready");
        assert_eq!(decoded.assertion().message(), "must be ready");
        assert_eq!(
            decoded.assertion().profile(),
            RuntimeAssertionProfile::Always
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

    #[test]
    fn runtime_assertion_codec_retains_guard_without_session_identity() {
        let failure = failure_fixture();

        let encoded = serde_json::to_vec(&failure).unwrap();
        let decoded: RuntimeAssertionFailure = serde_json::from_slice(&encoded).unwrap();

        assert_eq!(decoded, failure);
        assert_failure_payload(&decoded);
    }

    #[test]
    fn runtime_effect_transport_retains_guard_through_descriptor_and_materialization() {
        let guard = RuntimeAssertionGuardId::try_from_bytes([7; 16]).unwrap();
        let effect = RuntimeEffectExpr::Assert {
            guard,
            condition: value_expr(RuntimeValue::Bool(false)),
            message: "must be ready".to_owned(),
            profile: RuntimeAssertionProfile::Always,
        };

        let LineEffectRequest::Assert(descriptor) = effect.descriptor() else {
            panic!("assertion descriptor must retain its typed request kind");
        };
        assert_eq!(descriptor.guard(), guard);
        assert!(descriptor.condition().is_empty());
        assert_eq!(descriptor.message(), "must be ready");

        let Some(LineEffectRequest::Assert(materialized)) = effect
            .materialize(&[RuntimeValue::Bool(false)])
            .expect("assertion payload materializes")
        else {
            panic!("materialized assertion must retain its typed request kind");
        };
        assert_eq!(materialized.guard(), guard);
        assert_eq!(materialized.condition(), "false");
        assert_eq!(materialized.message(), "must be ready");
        assert_eq!(materialized.profile(), RuntimeAssertionProfile::Always);
    }

    #[test]
    fn successful_runtime_assertion_materializes_no_host_request() {
        let effect = RuntimeEffectExpr::Assert {
            guard: RuntimeAssertionGuardId::try_from_bytes([7; 16]).unwrap(),
            condition: value_expr(RuntimeValue::Bool(true)),
            message: "must be ready".to_owned(),
            profile: RuntimeAssertionProfile::Always,
        };

        let materialized = effect
            .materialize(&[RuntimeValue::Bool(true)])
            .expect("typed assertion payload is valid");

        assert_eq!(materialized, None);
    }

    #[test]
    fn non_bool_runtime_assertion_is_a_typed_materialization_error() {
        let effect = RuntimeEffectExpr::Assert {
            guard: RuntimeAssertionGuardId::try_from_bytes([7; 16]).unwrap(),
            condition: value_expr(RuntimeValue::String("false".to_owned())),
            message: "must be ready".to_owned(),
            profile: RuntimeAssertionProfile::Always,
        };

        let error = effect
            .materialize(&[RuntimeValue::String("false".to_owned())])
            .expect_err("string labels must not drive assertion truth");

        assert_eq!(
            error,
            RuntimeEffectMaterializeError::AssertionConditionNotBool
        );
    }
}
