//! Complete builtin schemas under the core-owned case and unary-payload ABI.

use crate::pattern::{RuntimeBuiltinVariantCaseSchema, RuntimeBuiltinVariantIdentity};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use super::RuntimeTypeSchema;

#[cfg(test)]
mod tests;

/// One item schema per payload-bearing case, in the owner's case order.
/// Unit cases and each one-item Tuple wrapper are derived from the same core
/// registry used by runtime value constructors; they are not caller-authored.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeBuiltinSchema {
    owner: RuntimeBuiltinVariantIdentity,
    payloads: Box<[RuntimeTypeSchema]>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("builtin schema {owner:?} requires {expected} payload item schemas, got {actual}")]
pub struct RuntimeBuiltinSchemaError {
    pub owner: RuntimeBuiltinVariantIdentity,
    pub expected: usize,
    pub actual: usize,
}

impl RuntimeBuiltinSchema {
    pub fn try_new(
        owner: RuntimeBuiltinVariantIdentity,
        payloads: impl Into<Box<[RuntimeTypeSchema]>>,
    ) -> Result<Self, RuntimeBuiltinSchemaError> {
        let payloads = payloads.into();
        if let Err(error) = Self::check_payload_count(owner, payloads.len()) {
            for payload in payloads {
                payload.drop_iteratively();
            }
            return Err(error);
        }
        Ok(Self { owner, payloads })
    }

    pub(crate) fn check_payload_count(
        owner: RuntimeBuiltinVariantIdentity,
        actual: usize,
    ) -> Result<(), RuntimeBuiltinSchemaError> {
        let expected = owner.payload_count();
        if actual == expected {
            Ok(())
        } else {
            Err(RuntimeBuiltinSchemaError {
                owner,
                expected,
                actual,
            })
        }
    }

    #[must_use]
    pub const fn owner(&self) -> RuntimeBuiltinVariantIdentity {
        self.owner
    }

    #[must_use]
    pub fn payloads(&self) -> &[RuntimeTypeSchema] {
        &self.payloads
    }

    /// Returns canonical case metadata and its inner payload item schema.
    #[must_use]
    pub fn case(
        &self,
        ordinal: usize,
    ) -> Option<(RuntimeBuiltinVariantCaseSchema, Option<&RuntimeTypeSchema>)> {
        let case = *self.owner.cases().get(ordinal)?;
        let payload = if case.has_payload() {
            let slot = self.owner.cases()[..ordinal]
                .iter()
                .filter(|case| case.has_payload())
                .count();
            Some(&self.payloads[slot])
        } else {
            None
        };
        Some((case, payload))
    }

    pub(super) fn into_payloads(self) -> Box<[RuntimeTypeSchema]> {
        self.payloads
    }
}

impl<'de> Deserialize<'de> for RuntimeBuiltinSchema {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Input {
            owner: RuntimeBuiltinVariantIdentity,
            payloads: Box<[RuntimeTypeSchema]>,
        }
        let input = Input::deserialize(deserializer)?;
        Self::try_new(input.owner, input.payloads).map_err(serde::de::Error::custom)
    }
}

impl RuntimeTypeSchema {
    pub fn builtin(
        owner: RuntimeBuiltinVariantIdentity,
        payloads: impl Into<Box<[Self]>>,
    ) -> Result<Self, RuntimeBuiltinSchemaError> {
        RuntimeBuiltinSchema::try_new(owner, payloads).map(Self::Builtin)
    }

    /// Instantiates the canonical Option schema.
    ///
    /// # Panics
    /// Panics only if the core Option registry ceases to have one payload case.
    #[must_use]
    pub fn option(item: Self) -> Self {
        Self::builtin(RuntimeBuiltinVariantIdentity::Option, vec![item])
            .expect("Option has one unary payload case")
    }

    /// Instantiates the canonical Result schema in Ok/Err order.
    ///
    /// # Panics
    /// Panics only if the core Result registry ceases to have two payload cases.
    #[must_use]
    pub fn result(ok: Self, error: Self) -> Self {
        Self::builtin(RuntimeBuiltinVariantIdentity::Result, vec![ok, error])
            .expect("Result has two unary payload cases")
    }
}
