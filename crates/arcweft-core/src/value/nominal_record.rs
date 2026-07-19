//! Nominal runtime record identity and ordered field storage.

use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use crate::value::RuntimeValue;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
}

impl RuntimeNominalRecordValue {
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
