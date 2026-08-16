//! Producer-owned runtime values for the accepted `Reduction<State>` family.

use crate::entry::{RuntimeCommandConstructorId, RuntimeCommandTargetId};
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use crate::value::{RuntimePayload, RuntimeValue};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

/// Core authority for the accepted opaque producer behind `Reduction<State>`.
///
/// The producer identity is compared only at this owning boundary. Consumers
/// use [`RuntimeReductionValue`] and never reconstruct the producer from a
/// source-level nominal spelling.
pub enum RuntimeReductionProducer {}

impl RuntimeReductionProducer {
    const ID: &'static str = "std.reduction";

    #[must_use]
    pub fn accepts(producer: &RuntimeOpaqueTypeProducerId) -> bool {
        producer.as_str() == Self::ID
    }

    #[must_use]
    pub fn accepts_exact_owner(owner: &RuntimeOpaqueTypeOwner) -> bool {
        owner.admission() == RuntimeOpaqueTypeAdmission::ExactIdentity
            && Self::accepts(owner.producer())
    }
}

/// Opaque replay-safe command produced by an admitted reduction constructor.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeCommand {
    constructor: RuntimeCommandConstructorId,
    target: RuntimeCommandTargetId,
    payload: RuntimePayload,
}

impl RuntimeCommand {
    pub(crate) const fn new_accepted(
        constructor: RuntimeCommandConstructorId,
        target: RuntimeCommandTargetId,
        payload: RuntimePayload,
    ) -> Self {
        Self {
            constructor,
            target,
            payload,
        }
    }

    #[must_use]
    pub const fn constructor(&self) -> &RuntimeCommandConstructorId {
        &self.constructor
    }

    #[must_use]
    pub const fn target(&self) -> &RuntimeCommandTargetId {
        &self.target
    }

    #[must_use]
    pub const fn payload(&self) -> &RuntimePayload {
        &self.payload
    }
}

/// Exact producer-owned value returned by a reducer.
///
/// The owner retains the complete semantic identity of `Reduction<State>`.
/// The generic `State` identity is not copied into the value: it remains a
/// child of the owning runtime-plan type projection and is checked when this
/// value is constructed by an admitted expression.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeReductionValue {
    owner: RuntimeOpaqueTypeOwner,
    state: Box<RuntimeValue>,
    commands: Box<[RuntimeCommand]>,
}

impl<'de> Deserialize<'de> for RuntimeReductionValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Repr {
            owner: RuntimeOpaqueTypeOwner,
            state: Box<RuntimeValue>,
            commands: Box<[RuntimeCommand]>,
        }

        let Repr {
            owner,
            state,
            commands,
        } = Repr::deserialize(deserializer)?;
        Self::try_from_admitted_parts(owner, *state, commands.into_vec()).map_err(D::Error::custom)
    }
}

impl RuntimeReductionValue {
    /// Constructs the result of the admitted `Reduction.unchanged` node.
    ///
    /// Expression admission is responsible for proving that `state` has the
    /// sole generic argument type of `owner`; this boundary independently
    /// rejects non-concrete and foreign opaque producers.
    pub(crate) fn try_unchanged(
        owner: RuntimeOpaqueTypeOwner,
        state: RuntimeValue,
    ) -> Result<Self, RuntimeReductionValueError> {
        Self::try_from_admitted_parts(owner, state, [])
    }

    /// Constructs a producer-owned reduction after the expression/domain
    /// admission layer has validated every command coordinate and payload.
    pub(crate) fn try_from_admitted_parts(
        owner: RuntimeOpaqueTypeOwner,
        state: RuntimeValue,
        commands: impl IntoIterator<Item = RuntimeCommand>,
    ) -> Result<Self, RuntimeReductionValueError> {
        if owner.admission() != RuntimeOpaqueTypeAdmission::ExactIdentity {
            return Err(RuntimeReductionValueError::NonConcreteOwner { owner });
        }
        if !RuntimeReductionProducer::accepts(owner.producer()) {
            return Err(RuntimeReductionValueError::WrongProducer {
                producer: owner.producer().clone(),
            });
        }
        Ok(Self {
            owner,
            state: Box::new(state),
            commands: commands.into_iter().collect(),
        })
    }

    #[must_use]
    pub const fn owner(&self) -> &RuntimeOpaqueTypeOwner {
        &self.owner
    }

    #[must_use]
    pub const fn state(&self) -> &RuntimeValue {
        &self.state
    }

    #[must_use]
    pub const fn commands(&self) -> &[RuntimeCommand] {
        &self.commands
    }

    #[must_use]
    pub fn into_parts(self) -> (RuntimeValue, Box<[RuntimeCommand]>) {
        (*self.state, self.commands)
    }
}

/// Failure to construct a concrete producer-owned reduction value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeReductionValueError {
    #[error("Reduction requires one exact opaque owner")]
    NonConcreteOwner { owner: RuntimeOpaqueTypeOwner },
    #[error("opaque producer `{producer:?}` cannot construct Reduction")]
    WrongProducer {
        producer: RuntimeOpaqueTypeProducerId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimeSemanticTypeId;

    fn producer(value: &str) -> RuntimeOpaqueTypeProducerId {
        RuntimeOpaqueTypeProducerId::try_new(value).expect("valid test producer")
    }

    #[test]
    fn unchanged_requires_the_exact_core_reduction_producer() {
        let semantic_identity = RuntimeSemanticTypeId::from_bytes([7; 32]);
        let exact = RuntimeOpaqueTypeOwner::exact(producer("std.reduction"), semantic_identity);
        let reduction = RuntimeReductionValue::try_unchanged(
            exact.clone(),
            RuntimeValue::String("state".to_owned()),
        )
        .expect("exact Reduction owner");
        assert_eq!(reduction.owner(), &exact);
        assert_eq!(reduction.state(), &RuntimeValue::String("state".to_owned()));
        assert!(reduction.commands().is_empty());

        let wide =
            RuntimeOpaqueTypeOwner::producer_wide(producer("std.reduction"), semantic_identity);
        assert!(matches!(
            RuntimeReductionValue::try_unchanged(wide, RuntimeValue::Unit),
            Err(RuntimeReductionValueError::NonConcreteOwner { .. })
        ));

        let foreign =
            RuntimeOpaqueTypeOwner::exact(producer("adapter.reduction"), semantic_identity);
        assert!(matches!(
            RuntimeReductionValue::try_unchanged(foreign, RuntimeValue::Unit),
            Err(RuntimeReductionValueError::WrongProducer { .. })
        ));
    }
}
