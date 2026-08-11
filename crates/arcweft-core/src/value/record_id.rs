//! Accepted runtime record-field identity.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, num::NonZeroU32};
use thiserror::Error;

/// One-based field identity in an accepted runtime record layout.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeRecordFieldId(NonZeroU32);

/// Failure to project an accepted record ordinal into the field-ID space.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeRecordFieldIdError {
    /// The accepted zero-based ordinal cannot be represented as a nonzero u32.
    #[error("runtime record field count exceeds u32 identity space")]
    OrdinalOverflow,
}

impl RuntimeRecordFieldId {
    #[allow(
        dead_code,
        reason = "record admission is blocked on the returned contract's missing schema owner"
    )]
    pub(crate) fn from_accepted_zero_based(
        ordinal: usize,
    ) -> Result<Self, RuntimeRecordFieldIdError> {
        u32::try_from(ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(1))
            .and_then(NonZeroU32::new)
            .map(Self)
            .ok_or(RuntimeRecordFieldIdError::OrdinalOverflow)
    }

    /// Returns the one-based accepted field identity.
    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }

    /// Returns the corresponding zero-based accepted storage ordinal.
    #[must_use]
    pub const fn zero_based(self) -> u32 {
        self.0.get() - 1
    }
}

impl fmt::Display for RuntimeRecordFieldId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for RuntimeRecordFieldId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0.get())
    }
}

impl<'de> Deserialize<'de> for RuntimeRecordFieldId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = RuntimeRecordFieldId;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a nonzero u32 runtime record field identity")
            }

            fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                NonZeroU32::new(value)
                    .map(RuntimeRecordFieldId)
                    .ok_or_else(|| E::custom("runtime record field identity must be nonzero"))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u32::try_from(value)
                    .map_err(E::custom)
                    .and_then(|value| self.visit_u32(value))
            }
        }

        deserializer.deserialize_u32(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_ordinals_are_one_based_and_checked() {
        let first = RuntimeRecordFieldId::from_accepted_zero_based(0).unwrap();
        assert_eq!(first.get().get(), 1);
        assert_eq!(first.zero_based(), 0);

        if usize::BITS > u32::BITS {
            assert_eq!(
                RuntimeRecordFieldId::from_accepted_zero_based(u32::MAX as usize),
                Err(RuntimeRecordFieldIdError::OrdinalOverflow)
            );
        }
    }

    #[test]
    fn serde_uses_nonzero_json_integer() {
        let field = RuntimeRecordFieldId::from_accepted_zero_based(1).unwrap();
        assert_eq!(serde_json::to_string(&field).unwrap(), "2");
        assert_eq!(
            serde_json::from_str::<RuntimeRecordFieldId>("2").unwrap(),
            field
        );
        assert!(serde_json::from_str::<RuntimeRecordFieldId>("0").is_err());
        assert!(serde_json::from_str::<RuntimeRecordFieldId>("\"2\"").is_err());
    }
}
