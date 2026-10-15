//! Closed storage roles for the existing live and inert value carriers.

use super::{AwbcRuntimeValueSnapshot, RuntimeValue, RuntimeValueView};
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};

/// Only the inert carrier can serialize owning retained-state grammars.
pub(crate) trait InertRuntimeValueStorage:
    serde::Serialize + serde::de::DeserializeOwned
{
}
impl InertRuntimeValueStorage for AwbcRuntimeValueSnapshot {}

/// Borrows the actual carrier and preserves its admission provenance.
pub(crate) trait RuntimeStoredValue {
    fn borrowed_view(&self) -> RuntimeValueView<'_>;
    fn admission_origin() -> ValueAdmissionOrigin;

    fn resource_admission(limits: RuntimeSchemaLimits) -> ValueResourceAdmission {
        match Self::admission_origin() {
            ValueAdmissionOrigin::Live => ValueResourceAdmission::live(limits),
            origin @ ValueAdmissionOrigin::InertSnapshot => {
                ValueResourceAdmission::snapshot(limits, origin)
            }
        }
    }
}

impl RuntimeStoredValue for RuntimeValue {
    fn borrowed_view(&self) -> RuntimeValueView<'_> {
        self.view()
    }

    fn admission_origin() -> ValueAdmissionOrigin {
        ValueAdmissionOrigin::Live
    }
}

impl RuntimeStoredValue for super::RuntimePayload {
    fn borrowed_view(&self) -> RuntimeValueView<'_> {
        self.value().view()
    }

    fn admission_origin() -> ValueAdmissionOrigin {
        ValueAdmissionOrigin::Live
    }
}

impl RuntimeStoredValue for AwbcRuntimeValueSnapshot {
    fn borrowed_view(&self) -> RuntimeValueView<'_> {
        self.view()
    }

    fn admission_origin() -> ValueAdmissionOrigin {
        ValueAdmissionOrigin::InertSnapshot
    }
}
