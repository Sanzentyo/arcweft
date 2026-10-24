//! The single lazy-context cause grammar and its inert-only codec.

use super::RuntimeArcErrorContextPending;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::{
    RuntimeFunctionEffectInstantiation, RuntimeProgramTypeError, RuntimeProgramValueType,
};
use crate::value::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeValueView};
use serde::de::{EnumAccess, VariantAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(test)]
mod tests;

impl<V> RuntimeArcErrorContextPending<V> {
    pub(crate) const fn kind(&self) -> super::RuntimeArcErrorContextKind {
        match self {
            Self::ResultErr(_) => super::RuntimeArcErrorContextKind::Result,
            Self::OptionNone => super::RuntimeArcErrorContextKind::Option,
        }
    }

    /// Validates the actual retained cause and the original source Result or
    /// Option role. The owning original row selects the payload type directly;
    /// no receiver, tuple, semantic-ID round trip or live capability is made.
    pub(crate) fn validate_source(
        &self,
        expected: RuntimeProgramValueType<'_>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), RuntimeProgramTypeError> {
        resources.work().charge(0)?;
        let case = match self {
            Self::ResultErr(_) => crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultErr,
            Self::OptionNone => crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionNone,
        };
        let payload = expected.builtin_case_payload(case, binding, resources)?;
        match (self, payload) {
            (Self::ResultErr(value), Some(payload)) => {
                payload.validate_view(view(value), binding, resources)
            }
            (Self::OptionNone, None) => Ok(()),
            _ => unreachable!("the original builtin case schema fixes payload presence"),
        }
    }
}

impl RuntimeArcErrorContextPending {
    /// The enclosing continuation has admitted its original source role and
    /// complete cause graph before any snapshot projection begins.
    pub(crate) fn snapshot_admitted(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<
        RuntimeArcErrorContextPending<AwbcRuntimeValueSnapshot>,
        AwbcRuntimeValueSnapshotError,
    > {
        Ok(match self {
            Self::ResultErr(value) => RuntimeArcErrorContextPending::ResultErr(
                AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)?,
            ),
            Self::OptionNone => RuntimeArcErrorContextPending::OptionNone,
        })
    }
}

impl RuntimeArcErrorContextPending<AwbcRuntimeValueSnapshot> {
    pub(crate) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> Result<RuntimeArcErrorContextPending, AwbcRuntimeValueSnapshotError> {
        let owner = owner.into();
        Ok(match self {
            Self::ResultErr(value) => {
                RuntimeArcErrorContextPending::ResultErr(value.into_admitted_runtime_value(owner)?)
            }
            Self::OptionNone => RuntimeArcErrorContextPending::OptionNone,
        })
    }
}

impl Serialize for RuntimeArcErrorContextPending<AwbcRuntimeValueSnapshot> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::ResultErr(value) => serializer.serialize_newtype_variant(
                "RuntimeArcErrorContextPending",
                0,
                "ResultErr",
                value,
            ),
            Self::OptionNone => {
                serializer.serialize_unit_variant("RuntimeArcErrorContextPending", 1, "OptionNone")
            }
        }
    }
}

impl<'de> Deserialize<'de> for RuntimeArcErrorContextPending<AwbcRuntimeValueSnapshot> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Private wire tags map exhaustively to the owning domain algebra.
        // There is deliberately no live Deserialize implementation.
        #[derive(Deserialize)]
        #[serde(field_identifier)]
        enum Case {
            ResultErr,
            OptionNone,
        }

        struct PendingVisitor;
        impl<'de> Visitor<'de> for PendingVisitor {
            type Value = RuntimeArcErrorContextPending<AwbcRuntimeValueSnapshot>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an inert lazy-context cause or absence")
            }
            fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Self::Value, A::Error> {
                let (case, variant) = data.variant::<Case>()?;
                match case {
                    Case::ResultErr => variant
                        .newtype_variant()
                        .map(RuntimeArcErrorContextPending::ResultErr),
                    Case::OptionNone => {
                        variant.unit_variant()?;
                        Ok(RuntimeArcErrorContextPending::OptionNone)
                    }
                }
            }
        }
        deserializer.deserialize_enum(
            "RuntimeArcErrorContextPending",
            &["ResultErr", "OptionNone"],
            PendingVisitor,
        )
    }
}
