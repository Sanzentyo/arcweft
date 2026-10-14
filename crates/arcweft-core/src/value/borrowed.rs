//! Read-only public inspection of the canonical logical value view.
//!
//! Private construction preserves live/inert provenance. These references own
//! no payload and grant no copying, activation or executable capability.

use super::{
    AwbcRuntimeValueSnapshot, RuntimeDialogueContentValue, RuntimeOpaqueValue, RuntimeOpaqueView,
    RuntimeScalarView, RuntimeSequenceView, RuntimeTupleView, RuntimeValue, RuntimeValueView,
};
use crate::{
    entry::schema::value_encoding::ValueAdmissionOrigin,
    pattern::{
        RuntimeBuiltinVariantCaseIdentity, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
    },
};

/// Borrowed inspection of one live value or inert snapshot and its descendants.
/// Inspection grants no conversion into a live owning carrier.
///
/// ```compile_fail
/// use arcweft_core::value::{RuntimeBorrowedValue, RuntimeValue};
/// fn activate(value: RuntimeBorrowedValue<'_>) -> RuntimeValue { value.into() }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct RuntimeBorrowedValue<'a> {
    pub(super) value: RuntimeValueView<'a>,
    pub(super) origin: ValueAdmissionOrigin,
}

/// Logical tuple fields borrowed in their owning codec order.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeBorrowedTuple<'a> {
    value: RuntimeTupleView<'a>,
    origin: ValueAdmissionOrigin,
}

/// Logical sequence rows, including dense and columnar storage.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeBorrowedSequence<'a> {
    value: RuntimeSequenceView<'a>,
    origin: ValueAdmissionOrigin,
}

/// Opaque header and payload inspection without activating a carrier.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeBorrowedOpaque<'a> {
    pub(super) value: RuntimeOpaqueView<'a>,
    pub(super) origin: ValueAdmissionOrigin,
}

/// Preserves a custom validator's failure separately from resource admission.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeBorrowedValueVisitError<E> {
    #[error(transparent)]
    Resource(Box<crate::entry::RuntimeSchemaError>),
    #[error("{0}")]
    Visitor(E),
}

impl<E> From<crate::entry::RuntimeSchemaError> for RuntimeBorrowedValueVisitError<E> {
    fn from(error: crate::entry::RuntimeSchemaError) -> Self {
        Self::Resource(Box::new(error))
    }
}

impl<'a> RuntimeBorrowedValue<'a> {
    /// Visits the same logical codec graph after resource preflight. Traversal
    /// never constructs live values from inert or domain-owned storage.
    pub fn try_visit<E>(
        self,
        limits: crate::entry::RuntimeSchemaLimits,
        mut visit: impl for<'value> FnMut(RuntimeBorrowedValue<'value>) -> Result<(), E>,
    ) -> Result<(), RuntimeBorrowedValueVisitError<E>> {
        let work = crate::entry::schema::value_encoding::preflight_snapshot_view(
            self.value, limits, None,
        )?;
        crate::entry::schema::value_encoding::visit_admitted_storage(self.value, &work, |value| {
            visit(RuntimeBorrowedValue {
                value,
                origin: self.origin,
            })
            .map_err(RuntimeBorrowedValueVisitError::Visitor)
        })
    }
    fn child(self, value: RuntimeValueView<'a>) -> Self {
        Self {
            value,
            origin: self.origin,
        }
    }
    /// Returns a scalar atom without normalizing its integer or float width.
    #[must_use]
    pub fn scalar(self) -> Option<RuntimeScalarView<'a>> {
        match self.value {
            RuntimeValueView::Scalar(value) => Some(value),
            _ => None,
        }
    }
    /// Borrows logical tuple fields without allocating an intermediate row.
    #[must_use]
    pub fn tuple(self) -> Option<RuntimeBorrowedTuple<'a>> {
        match self.value {
            RuntimeValueView::Tuple(value) => Some(RuntimeBorrowedTuple {
                value,
                origin: self.origin,
            }),
            _ => None,
        }
    }
    /// Borrows sequence storage and its logical rows.
    #[must_use]
    pub fn sequence(self) -> Option<RuntimeBorrowedSequence<'a>> {
        match self.value {
            RuntimeValueView::Sequence(value) => Some(RuntimeBorrowedSequence {
                value,
                origin: self.origin,
            }),
            _ => None,
        }
    }
    /// Borrows a header without treating the candidate as type evidence.
    #[must_use]
    pub fn opaque(self) -> Option<RuntimeBorrowedOpaque<'a>> {
        match self.value {
            RuntimeValueView::Opaque(value) => Some(RuntimeBorrowedOpaque {
                value,
                origin: self.origin,
            }),
            _ => None,
        }
    }
    /// Inspects an exact builtin case through the existing canonical grammar.
    #[must_use]
    pub fn builtin_variant_case(self) -> Option<(RuntimeBuiltinVariantCaseIdentity, Option<Self>)> {
        self.value
            .builtin_variant_case()
            .map(|(case, payload)| (case, payload.map(|value| self.child(value))))
    }
}

impl<'a> RuntimeBorrowedTuple<'a> {
    #[must_use]
    pub fn len(self) -> usize {
        self.value.len()
    }
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    #[must_use]
    pub fn get(self, index: usize) -> Option<RuntimeBorrowedValue<'a>> {
        self.value.get(index).map(|value| RuntimeBorrowedValue {
            value,
            origin: self.origin,
        })
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = RuntimeBorrowedValue<'a>> {
        self.value.iter().map(move |value| RuntimeBorrowedValue {
            value,
            origin: self.origin,
        })
    }
}

impl<'a> RuntimeBorrowedSequence<'a> {
    pub fn len(self) -> Result<usize, crate::entry::RuntimeSchemaError> {
        self.value.len()
    }
    pub fn is_empty(self) -> Result<bool, crate::entry::RuntimeSchemaError> {
        self.len().map(|len| len == 0)
    }
    #[must_use]
    pub fn get(self, index: usize) -> Option<RuntimeBorrowedValue<'a>> {
        self.value
            .value_view(index)
            .map(|value| RuntimeBorrowedValue {
                value,
                origin: self.origin,
            })
    }
    /// Exact dense byte storage; an ordinary numeric sequence is not retyped.
    #[must_use]
    pub fn dense_bytes(self) -> Option<&'a [u8]> {
        self.value.dense_bytes()
    }
}

impl<'a> RuntimeBorrowedOpaque<'a> {
    #[must_use]
    pub fn producer(self) -> &'a RuntimeOpaqueTypeProducerId {
        self.value.producer()
    }
    #[must_use]
    pub fn semantic_identity(self) -> RuntimeSemanticTypeId {
        self.value.semantic_identity()
    }
    #[must_use]
    pub fn value_class(self) -> super::RuntimeOpaqueValueClass {
        self.value.value_class()
    }
    #[must_use]
    pub fn persistence(self) -> super::RuntimeOpaquePersistence {
        self.value.persistence()
    }
    #[must_use]
    pub fn payload(self) -> RuntimeBorrowedValue<'a> {
        RuntimeBorrowedValue {
            value: self.value.payload(),
            origin: self.origin,
        }
    }
}

impl<'a> From<&'a RuntimeValue> for RuntimeBorrowedValue<'a> {
    fn from(value: &'a RuntimeValue) -> Self {
        Self {
            value: value.view(),
            origin: ValueAdmissionOrigin::Live,
        }
    }
}
impl<'a> From<&'a RuntimeOpaqueValue> for RuntimeBorrowedValue<'a> {
    fn from(value: &'a RuntimeOpaqueValue) -> Self {
        Self {
            value: value.view(),
            origin: ValueAdmissionOrigin::Live,
        }
    }
}
impl<'a> From<&'a RuntimeDialogueContentValue> for RuntimeBorrowedValue<'a> {
    fn from(value: &'a RuntimeDialogueContentValue) -> Self {
        Self {
            value: value.view(),
            origin: ValueAdmissionOrigin::Live,
        }
    }
}
impl<'a> From<&'a AwbcRuntimeValueSnapshot> for RuntimeBorrowedValue<'a> {
    fn from(value: &'a AwbcRuntimeValueSnapshot) -> Self {
        Self {
            value: value.view(),
            origin: ValueAdmissionOrigin::InertSnapshot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_borrows_live_and_inert_rows_without_retyping_or_copying_text() {
        let text = RuntimeValue::String("borrowed".to_owned());
        let RuntimeValue::String(source) = &text else {
            unreachable!()
        };
        let Some(RuntimeScalarView::String(value)) = RuntimeBorrowedValue::from(&text).scalar()
        else {
            panic!("borrowed String")
        };
        assert!(std::ptr::eq(source.as_ptr(), value.as_ptr()));
        let inert = AwbcRuntimeValueSnapshot::Tuple(vec![
            AwbcRuntimeValueSnapshot::UInt(super::super::RuntimeUInt::U16(7)),
            AwbcRuntimeValueSnapshot::String("inert".to_owned()),
        ]);
        let tuple = RuntimeBorrowedValue::from(&inert).tuple().unwrap();
        assert_eq!(tuple.len(), 2);
        assert_eq!(
            tuple.get(0).unwrap().scalar(),
            Some(RuntimeScalarView::UInt(super::super::RuntimeUInt::U16(7)))
        );
        assert!(matches!(
            tuple.get(1).unwrap().origin,
            ValueAdmissionOrigin::InertSnapshot
        ));
        let AwbcRuntimeValueSnapshot::Tuple(fields) = &inert else {
            unreachable!()
        };
        let AwbcRuntimeValueSnapshot::String(source) = &fields[1] else {
            unreachable!()
        };
        let Some(RuntimeScalarView::String(value)) = tuple.get(1).unwrap().scalar() else {
            panic!("inert String")
        };
        assert!(std::ptr::eq(source.as_ptr(), value.as_ptr()));
    }

    #[test]
    fn borrowed_graph_visit_preserves_custom_failures_and_resource_limits() {
        let value = AwbcRuntimeValueSnapshot::Tuple(vec![AwbcRuntimeValueSnapshot::String(
            "leaf".to_owned(),
        )]);
        let mut visited = 0;
        let result = RuntimeBorrowedValue::from(&value).try_visit(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            |_| {
                visited += 1;
                Err::<(), _>("catalog failure")
            },
        );
        assert!(matches!(
            result,
            Err(RuntimeBorrowedValueVisitError::Visitor("catalog failure"))
        ));
        assert_eq!(visited, 1);
        let result = RuntimeBorrowedValue::from(&value).try_visit(
            crate::entry::RuntimeSchemaLimits {
                max_nodes: 1,
                ..crate::entry::RuntimeSchemaLimits::engine_default()
            },
            |_| {
                visited += 1;
                Ok::<(), &str>(())
            },
        );
        assert!(matches!(
            result,
            Err(RuntimeBorrowedValueVisitError::Resource(_))
        ));
        assert_eq!(visited, 1);
    }
}
