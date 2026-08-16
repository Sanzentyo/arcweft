//! Nominal runtime record identity and ordered field storage.

use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::value::RuntimeValue;
use crate::value::{RuntimeRecordFieldId, RuntimeRecordFieldIdError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Immutable executable field layout for one checked nominal record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalRecordLayout {
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
    fields: Box<[RuntimeNominalRecordLayoutField]>,
}

/// One defining-order field in an executable nominal record layout.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalRecordLayoutField {
    name: String,
    checked_type: RuntimeCheckedType,
}

/// Failure to admit an executable nominal record layout.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordLayoutError {
    #[error(
        "nominal record layout has {actual} fields, exceeding the {maximum}-field identity space"
    )]
    TooManyFields { actual: usize, maximum: u32 },
    #[error("nominal record layout contains duplicate field `{name}`")]
    DuplicateFieldName { name: String },
    #[error("nominal record layout field {ordinal} (`{name}`) has invalid identity")]
    InvalidFieldIdentity {
        ordinal: usize,
        name: String,
        source: RuntimeRecordFieldIdError,
    },
}

impl RuntimeNominalRecordLayout {
    /// Admits one complete checked defining-order field projection.
    pub fn try_from_checked_projection(
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        fields_in_layout_order: Vec<(String, RuntimeCheckedType)>,
    ) -> Result<Self, RuntimeNominalRecordLayoutError> {
        if fields_in_layout_order.len() > u32::MAX as usize {
            return Err(RuntimeNominalRecordLayoutError::TooManyFields {
                actual: fields_in_layout_order.len(),
                maximum: u32::MAX,
            });
        }

        let mut names = BTreeSet::new();
        for (name, _) in &fields_in_layout_order {
            if !names.insert(name.as_str()) {
                return Err(RuntimeNominalRecordLayoutError::DuplicateFieldName {
                    name: name.clone(),
                });
            }
        }

        for (ordinal, (name, _)) in fields_in_layout_order.iter().enumerate() {
            RuntimeRecordFieldId::from_accepted_zero_based(ordinal).map_err(|source| {
                RuntimeNominalRecordLayoutError::InvalidFieldIdentity {
                    ordinal,
                    name: name.clone(),
                    source,
                }
            })?;
        }

        Ok(Self {
            nominal,
            semantic_identity,
            layout,
            fields: fields_in_layout_order
                .into_iter()
                .map(|(name, checked_type)| RuntimeNominalRecordLayoutField { name, checked_type })
                .collect(),
        })
    }

    /// Canonical runtime nominal identity.
    #[must_use]
    pub const fn nominal(&self) -> &RuntimeNominalTypeId {
        &self.nominal
    }

    /// Checked semantic projection identity.
    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    /// Exact transitive runtime layout identity.
    #[must_use]
    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }

    /// Fields in defining layout order.
    #[must_use]
    pub fn fields(&self) -> &[RuntimeNominalRecordLayoutField] {
        &self.fields
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Derives the accepted field identity for one defining-order ordinal.
    #[must_use]
    pub fn field_id(&self, zero_based_ordinal: usize) -> Option<RuntimeRecordFieldId> {
        if zero_based_ordinal >= self.fields.len() {
            return None;
        }
        RuntimeRecordFieldId::from_accepted_zero_based(zero_based_ordinal).ok()
    }

    #[must_use]
    pub fn field_by_id(
        &self,
        field: RuntimeRecordFieldId,
    ) -> Option<&RuntimeNominalRecordLayoutField> {
        usize::try_from(field.zero_based())
            .ok()
            .and_then(|ordinal| self.fields.get(ordinal))
    }

    #[must_use]
    pub fn field_by_name(
        &self,
        name: &str,
    ) -> Option<(RuntimeRecordFieldId, &RuntimeNominalRecordLayoutField)> {
        let (ordinal, field) = self
            .fields
            .iter()
            .enumerate()
            .find(|(_, field)| field.name == name)?;
        self.field_id(ordinal).map(|identity| (identity, field))
    }

    /// Closed checked predicate corresponding to this descriptor.
    #[must_use]
    pub fn checked_type(&self) -> RuntimeCheckedType {
        RuntimeCheckedType::Nominal {
            nominal: self.nominal.clone(),
            semantic_identity: self.semantic_identity,
            layout: self.layout,
        }
    }
}

impl RuntimeNominalRecordLayoutField {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn checked_type(&self) -> &RuntimeCheckedType {
        &self.checked_type
    }
}

/// Runtime value for one nominal record with schema-ordinal fields.
///
/// Unlike [`RuntimeValue::Record`], the fields have no source names and retain
/// their defining nominal type and exact layout identity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeNominalRecordValue {
    type_id: RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    fields: Vec<RuntimeValue>,
}

/// Failure to use a nominal record under an expected runtime schema.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeNominalRecordError {
    #[error("expected nominal type `{expected:?}`, found `{actual:?}`")]
    Type {
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },
    #[error("nominal record layout does not match the expected layout")]
    Layout {
        expected: TypeLayoutHash,
        actual: TypeLayoutHash,
    },
    #[error("nominal record has {actual} fields, expected {expected}")]
    FieldCount { expected: usize, actual: usize },
    #[error("nominal record layout ordinal {ordinal} has invalid field identity")]
    InvalidFieldIdentity {
        ordinal: usize,
        source: RuntimeRecordFieldIdError,
    },
    #[error("nominal record field {field:?} (`{name}`) does not satisfy {expected:?}")]
    FieldType {
        field: RuntimeRecordFieldId,
        name: String,
        expected: RuntimeCheckedType,
    },
}

impl RuntimeNominalRecordValue {
    /// Constructs a value from fields already arranged in defining-layout order.
    pub(crate) fn try_from_accepted_layout(
        layout: &RuntimeNominalRecordLayout,
        fields_in_layout_order: Vec<RuntimeValue>,
    ) -> Result<Self, RuntimeNominalRecordError> {
        validate_layout_fields(layout, &fields_in_layout_order)?;
        Ok(Self {
            type_id: layout.nominal().clone(),
            layout: layout.layout(),
            fields: fields_in_layout_order,
        })
    }

    /// Constructs an already schema-ordered nominal record.
    #[must_use]
    pub const fn new(
        type_id: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        fields: Vec<RuntimeValue>,
    ) -> Self {
        Self {
            type_id,
            layout,
            fields,
        }
    }

    /// Stable nominal type identity.
    #[must_use]
    pub const fn type_id(&self) -> &RuntimeNominalTypeId {
        &self.type_id
    }

    /// Exact transitive type-layout identity.
    #[must_use]
    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }

    /// Values in defining schema-ordinal order.
    #[must_use]
    pub fn fields(&self) -> &[RuntimeValue] {
        &self.fields
    }

    /// Consumes the carrier and returns its ordered fields.
    #[must_use]
    pub fn into_fields(self) -> Vec<RuntimeValue> {
        self.fields
    }

    /// Derives the accepted field identity for one stored ordinal.
    #[must_use]
    pub fn field_id(&self, zero_based_ordinal: usize) -> Option<RuntimeRecordFieldId> {
        if zero_based_ordinal >= self.fields.len() {
            return None;
        }
        RuntimeRecordFieldId::from_accepted_zero_based(zero_based_ordinal).ok()
    }

    /// Reads a stored field by its accepted one-based identity.
    #[must_use]
    pub fn field(&self, field: RuntimeRecordFieldId) -> Option<&RuntimeValue> {
        usize::try_from(field.zero_based())
            .ok()
            .and_then(|ordinal| self.fields.get(ordinal))
    }

    /// Replaces one field selected by its accepted defining-order identity.
    ///
    /// The caller retains the supplied value when the identity is outside this
    /// record's admitted layout.
    pub(crate) fn replace_field(
        &mut self,
        field: RuntimeRecordFieldId,
        value: RuntimeValue,
    ) -> Result<(), RuntimeValue> {
        let Some(slot) = usize::try_from(field.zero_based())
            .ok()
            .and_then(|ordinal| self.fields.get_mut(ordinal))
        else {
            return Err(value);
        };
        *slot = value;
        Ok(())
    }

    /// Validates a restored or otherwise pre-existing value against one layout.
    pub fn validate_against_layout(
        &self,
        layout: &RuntimeNominalRecordLayout,
    ) -> Result<(), RuntimeNominalRecordError> {
        if self.type_id() != layout.nominal() {
            return Err(RuntimeNominalRecordError::Type {
                expected: layout.nominal().clone(),
                actual: self.type_id().clone(),
            });
        }
        if self.layout() != layout.layout() {
            return Err(RuntimeNominalRecordError::Layout {
                expected: layout.layout(),
                actual: self.layout(),
            });
        }
        validate_layout_fields(layout, self.fields())
    }

    /// Verifies identity, layout, and exact schema field count together.
    pub fn validate_shape(
        &self,
        expected_type: &RuntimeNominalTypeId,
        expected_layout: TypeLayoutHash,
        expected_fields: usize,
    ) -> Result<(), RuntimeNominalRecordError> {
        if &self.type_id != expected_type {
            return Err(RuntimeNominalRecordError::Type {
                expected: expected_type.clone(),
                actual: self.type_id.clone(),
            });
        }
        if self.layout != expected_layout {
            return Err(RuntimeNominalRecordError::Layout {
                expected: expected_layout,
                actual: self.layout,
            });
        }
        if self.fields.len() != expected_fields {
            return Err(RuntimeNominalRecordError::FieldCount {
                expected: expected_fields,
                actual: self.fields.len(),
            });
        }
        Ok(())
    }
}

fn validate_layout_fields(
    layout: &RuntimeNominalRecordLayout,
    fields: &[RuntimeValue],
) -> Result<(), RuntimeNominalRecordError> {
    if fields.len() != layout.len() {
        return Err(RuntimeNominalRecordError::FieldCount {
            expected: layout.len(),
            actual: fields.len(),
        });
    }
    for (ordinal, (field_layout, value)) in layout.fields().iter().zip(fields).enumerate() {
        let field = RuntimeRecordFieldId::from_accepted_zero_based(ordinal).map_err(|source| {
            RuntimeNominalRecordError::InvalidFieldIdentity { ordinal, source }
        })?;
        if !field_layout.checked_type().accepts_value(value) {
            return Err(RuntimeNominalRecordError::FieldType {
                field,
                name: field_layout.name().to_owned(),
                expected: field_layout.checked_type().clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn layout(fields: Vec<(String, RuntimeCheckedType)>) -> RuntimeNominalRecordLayout {
        RuntimeNominalRecordLayout::try_from_checked_projection(
            RuntimeNominalTypeId::try_new("game.State").expect("nominal identity"),
            RuntimeSemanticTypeId::from_bytes([3; 32]),
            TypeLayoutHash::from_bytes([5; 32]),
            fields,
        )
        .expect("accepted layout")
    }

    #[test]
    fn nominal_layout_preserves_defining_order_and_derives_field_ids() {
        let layout = layout(vec![
            ("alpha".to_owned(), RuntimeCheckedType::Bool),
            ("zeta".to_owned(), RuntimeCheckedType::String),
        ]);

        assert_eq!(layout.fields()[0].name(), "alpha");
        assert_eq!(layout.fields()[1].name(), "zeta");
        assert_eq!(layout.field_id(0).map(|field| field.get().get()), Some(1));
        assert_eq!(layout.field_id(1).map(|field| field.get().get()), Some(2));
        assert_eq!(
            layout
                .field_by_name("zeta")
                .map(|(identity, _)| identity.get().get()),
            Some(2)
        );
        assert!(layout.field_id(2).is_none());
    }

    #[test]
    fn nominal_layout_is_structurally_equal_across_distinct_allocations() {
        let first = Arc::new(layout(vec![("value".to_owned(), RuntimeCheckedType::Bool)]));
        let second = Arc::new(layout(vec![("value".to_owned(), RuntimeCheckedType::Bool)]));

        assert_eq!(first, second);
        assert!(!Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn nominal_layout_rejects_first_duplicate_name() {
        let error = RuntimeNominalRecordLayout::try_from_checked_projection(
            RuntimeNominalTypeId::try_new("game.State").expect("nominal identity"),
            RuntimeSemanticTypeId::from_bytes([3; 32]),
            TypeLayoutHash::from_bytes([5; 32]),
            vec![
                ("value".to_owned(), RuntimeCheckedType::Bool),
                ("value".to_owned(), RuntimeCheckedType::String),
            ],
        )
        .expect_err("duplicate field names reject");

        assert_eq!(
            error,
            RuntimeNominalRecordLayoutError::DuplicateFieldName {
                name: "value".to_owned()
            }
        );
    }

    #[test]
    fn nominal_checked_type_requires_exact_layout_hash() {
        let layout = layout(Vec::new());
        let value = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            layout.nominal().clone(),
            layout.layout(),
            Vec::new(),
        ));
        assert!(layout.checked_type().accepts_value(&value));

        let wrong = RuntimeCheckedType::Nominal {
            nominal: layout.nominal().clone(),
            semantic_identity: layout.semantic_identity(),
            layout: TypeLayoutHash::from_bytes([9; 32]),
        };
        assert!(!wrong.accepts_value(&value));
    }
}
