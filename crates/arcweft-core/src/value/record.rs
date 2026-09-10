//! Ordered structural record storage and its field inventory.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::{RecordSeq, RuntimeRecordFieldId, RuntimeRecordFieldIdError, RuntimeValue};

/// Failure to construct an ordered structural record field inventory.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeRecordAdmissionError {
    #[error("runtime record has duplicate field name `{name}`")]
    DuplicateName { name: String },
    #[error("runtime record field {field} has an empty name")]
    EmptyName { field: RuntimeRecordFieldId },
    #[error("runtime record has too many fields")]
    TooManyFields,
    #[error("runtime record field `{name}` has invalid identity")]
    InvalidFieldIdentity {
        name: String,
        #[source]
        source: RuntimeRecordFieldIdError,
    },
    #[error("runtime record field {actual} does not match storage coordinate {expected}")]
    FieldOrder {
        expected: RuntimeRecordFieldId,
        actual: RuntimeRecordFieldId,
    },
}

/// One immutable field coordinate and its value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeFieldValue {
    field: RuntimeRecordFieldId,
    name: String,
    value: RuntimeValue,
}

impl RuntimeFieldValue {
    pub(crate) const fn new_accepted(
        field: RuntimeRecordFieldId,
        name: String,
        value: RuntimeValue,
    ) -> Self {
        Self { field, name, value }
    }

    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub const fn value(&self) -> &RuntimeValue {
        &self.value
    }
    pub(crate) const fn value_mut(&mut self) -> &mut RuntimeValue {
        &mut self.value
    }
    pub(crate) fn into_value(self) -> RuntimeValue {
        self.value
    }
}

/// Structural record data with contiguous field IDs and unique nonempty names.
///
/// This owns storage validity. Admission against an executable declaration or
/// persistence schema remains the responsibility of that typed owner.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeRecordValue {
    fields: Vec<RuntimeFieldValue>,
}

/// One ordered field admission shared by row and column storage.
pub(super) struct RecordFieldAdmission<'a> {
    ordinal: usize,
    names: BTreeSet<&'a str>,
}

impl<'a> RecordFieldAdmission<'a> {
    pub(super) fn new(count: usize) -> Result<Self, RuntimeRecordAdmissionError> {
        RuntimeRecordValue::check_field_count(count)?;
        Ok(Self {
            ordinal: 0,
            names: BTreeSet::new(),
        })
    }

    pub(super) fn admit(
        &mut self,
        field: RuntimeRecordFieldId,
        name: &'a str,
    ) -> Result<(), RuntimeRecordAdmissionError> {
        let expected = RuntimeRecordValue::field_identity_at(self.ordinal, name)?;
        if field != expected {
            return Err(RuntimeRecordAdmissionError::FieldOrder {
                expected,
                actual: field,
            });
        }
        if name.is_empty() {
            return Err(RuntimeRecordAdmissionError::EmptyName { field });
        }
        if !self.names.insert(name) {
            return Err(RuntimeRecordAdmissionError::DuplicateName {
                name: name.to_owned(),
            });
        }
        self.ordinal += 1;
        Ok(())
    }
}

impl RuntimeRecordValue {
    /// Defines an anonymous record's field inventory in the supplied order.
    pub fn try_new(
        fields_in_order: Vec<(String, RuntimeValue)>,
    ) -> Result<Self, RuntimeRecordAdmissionError> {
        Self::check_field_count(fields_in_order.len())?;
        let fields = fields_in_order
            .into_iter()
            .enumerate()
            .map(|(ordinal, (name, value))| {
                let field = Self::field_identity_at(ordinal, &name)?;
                Ok(RuntimeFieldValue::new_accepted(field, name, value))
            })
            .collect::<Result<Vec<_>, RuntimeRecordAdmissionError>>()?;
        Self::try_from_fields(fields)
    }

    pub(crate) fn check_field_count(count: usize) -> Result<(), RuntimeRecordAdmissionError> {
        if count > u32::MAX as usize {
            return Err(RuntimeRecordAdmissionError::TooManyFields);
        }
        Ok(())
    }

    pub(crate) fn field_identity_at(
        ordinal: usize,
        name: &str,
    ) -> Result<RuntimeRecordFieldId, RuntimeRecordAdmissionError> {
        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).map_err(|source| {
            RuntimeRecordAdmissionError::InvalidFieldIdentity {
                name: name.to_owned(),
                source,
            }
        })
    }

    pub(crate) fn try_from_fields(
        fields: Vec<RuntimeFieldValue>,
    ) -> Result<Self, RuntimeRecordAdmissionError> {
        let mut admission = RecordFieldAdmission::new(fields.len())?;
        for field in &fields {
            admission.admit(field.field, &field.name)?;
        }
        Ok(Self { fields })
    }

    /// The admitted column layout supplies every field coordinate unchanged.
    pub(super) fn from_sequence_row(sequence: &RecordSeq, row: usize) -> Self {
        assert!(row < sequence.len(), "record row is in bounds");
        Self {
            fields: sequence
                .fields()
                .iter()
                .map(|field| {
                    RuntimeFieldValue::new_accepted(
                        field.field(),
                        field.name().to_owned(),
                        field.values().value_at(row),
                    )
                })
                .collect(),
        }
    }

    pub fn fields(&self) -> &[RuntimeFieldValue] {
        &self.fields
    }
    pub fn iter(&self) -> std::slice::Iter<'_, RuntimeFieldValue> {
        self.fields.iter()
    }
    pub fn len(&self) -> usize {
        self.fields.len()
    }
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
    pub fn get(&self, ordinal: usize) -> Option<&RuntimeFieldValue> {
        self.fields.get(ordinal)
    }
    pub(crate) fn same_layout(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(left, right)| left.field == right.field && left.name == right.name)
    }

    pub(crate) fn field_value_mut(
        &mut self,
        field: RuntimeRecordFieldId,
    ) -> Option<&mut RuntimeValue> {
        self.fields
            .get_mut(field.zero_based() as usize)
            .map(RuntimeFieldValue::value_mut)
    }

    /// Transforms values while preserving the field inventory exactly.
    pub fn try_map_values<E>(
        self,
        mut map: impl FnMut(RuntimeValue) -> Result<RuntimeValue, E>,
    ) -> Result<Self, E> {
        let fields = self
            .fields
            .into_iter()
            .map(|field| {
                let RuntimeFieldValue { field, name, value } = field;
                map(value).map(|value| RuntimeFieldValue::new_accepted(field, name, value))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { fields })
    }
}

impl<'de> Deserialize<'de> for RuntimeRecordValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from_fields(Vec::<RuntimeFieldValue>::deserialize(deserializer)?)
            .map_err(de::Error::custom)
    }
}

impl IntoIterator for RuntimeRecordValue {
    type Item = RuntimeFieldValue;
    type IntoIter = std::vec::IntoIter<RuntimeFieldValue>;
    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<'a> IntoIterator for &'a RuntimeRecordValue {
    type Item = &'a RuntimeFieldValue;
    type IntoIter = std::slice::Iter<'a, RuntimeFieldValue>;
    fn into_iter(self) -> Self::IntoIter {
        self.fields.iter()
    }
}

impl RuntimeValue {
    pub fn try_record(
        fields_in_order: Vec<(String, RuntimeValue)>,
    ) -> Result<Self, RuntimeRecordAdmissionError> {
        RuntimeRecordValue::try_new(fields_in_order).map(Self::Record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value() -> RuntimeValue {
        RuntimeValue::try_record(vec![
            ("z".to_owned(), RuntimeValue::Bool(true)),
            ("a".to_owned(), RuntimeValue::Bool(false)),
        ])
        .unwrap()
    }

    #[test]
    fn serde_rejects_invalid_record_inventory_before_issuing_a_value() {
        let accepted = value();
        let encoded = serde_json::to_value(&accepted).unwrap();
        assert_eq!(
            serde_json::from_value::<RuntimeValue>(encoded.clone()).unwrap(),
            accepted
        );
        for (ordinal, field, value) in [
            (0, "field", serde_json::json!(0)),
            (0, "field", serde_json::json!(2)),
            (1, "field", serde_json::json!(1)),
            (1, "field", serde_json::json!(3)),
            (0, "name", serde_json::json!("")),
            (1, "name", serde_json::json!("z")),
        ] {
            let mut malformed = encoded.clone();
            malformed["Record"][ordinal][field] = value;
            assert!(serde_json::from_value::<RuntimeValue>(malformed).is_err());
        }
        let mut reordered = encoded;
        reordered["Record"].as_array_mut().unwrap().swap(0, 1);
        assert!(serde_json::from_value::<RuntimeValue>(reordered).is_err());
    }

    #[test]
    fn mapping_values_preserves_the_inventory_and_stops_on_failure() {
        let RuntimeValue::Record(record) = value() else {
            panic!("record");
        };
        let identity = record
            .iter()
            .map(|field| (field.field(), field.name().to_owned()))
            .collect::<Vec<_>>();
        let mapped = record
            .clone()
            .try_map_values::<()>(|_| Ok(RuntimeValue::Unit))
            .unwrap();
        assert_eq!(
            mapped
                .iter()
                .map(|field| (field.field(), field.name().to_owned()))
                .collect::<Vec<_>>(),
            identity
        );
        assert!(
            mapped
                .iter()
                .all(|field| field.value() == &RuntimeValue::Unit)
        );
        let mut calls = 0;
        let rejected = record.try_map_values(|_| {
            calls += 1;
            Err("rejected")
        });
        assert_eq!(rejected, Err("rejected"));
        assert_eq!(calls, 1);
    }

    #[test]
    fn named_construction_rejects_empty_names_and_accepts_an_empty_record() {
        let first = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
        assert_eq!(
            RuntimeRecordValue::try_new(vec![(String::new(), RuntimeValue::Unit)]),
            Err(RuntimeRecordAdmissionError::EmptyName { field: first })
        );
        assert!(RuntimeRecordValue::default().is_empty());
    }
}
