//! Missing-field requests and a Sans-I/O default-provider context.
//!
//! Shape metadata declares when a default is required. Its owning producer
//! supplies the value; codecs never guess one from the field's type.

use std::borrow::Cow;

use crate::{
    DataError, DataErrorKind, FieldShape, Result, ShapeAccess, ShapeId, ShapeRef, TypeShape, Value,
};

/// Exact record occurrence and declaration-order field requested by a decoder.
/// This coordinate is not itself permission to execute a runtime callable.
#[derive(Clone, Copy, Debug)]
pub struct FieldDefaultRequest<'shape> {
    record: ShapeRef<'shape>,
    field_ordinal: usize,
}

impl<'shape> FieldDefaultRequest<'shape> {
    #[must_use]
    pub const fn new(record: ShapeRef<'shape>, field_ordinal: usize) -> Self {
        Self {
            record,
            field_ordinal,
        }
    }

    #[must_use]
    pub const fn record(&self) -> ShapeRef<'shape> {
        self.record
    }

    #[must_use]
    pub const fn field_ordinal(&self) -> usize {
        self.field_ordinal
    }

    /// Graph-backed runtime providers require this coordinate; inline Rust
    /// callers may instead resolve the supplied record under their own owner.
    #[must_use]
    pub const fn record_id(&self) -> Option<ShapeId> {
        self.record.referenced_id()
    }
}

/// Owner of admitted constants or pure default callables. Runtime providers
/// validate their generated value against the selected field type before
/// returning it; the data layer additionally checks the reflected value shape.
pub trait FieldDefaultProvider {
    fn default_value(&self, request: FieldDefaultRequest<'_>) -> Result<Value>;
}

/// Borrowed decoding context pairing one unchanged shape graph with its
/// explicit provider. It owns neither a descriptor graph nor a default catalog.
#[derive(Clone, Copy)]
pub struct DecodeShapeAccess<'a> {
    shapes: &'a dyn ShapeAccess,
    defaults: &'a dyn FieldDefaultProvider,
}

impl<'a> DecodeShapeAccess<'a> {
    #[must_use]
    pub const fn new(shapes: &'a dyn ShapeAccess, defaults: &'a dyn FieldDefaultProvider) -> Self {
        Self { shapes, defaults }
    }
}

impl ShapeAccess for DecodeShapeAccess<'_> {
    fn get_shape(&self, id: ShapeId) -> Option<Cow<'_, TypeShape>> {
        self.shapes.get_shape(id)
    }

    fn field_defaults(&self) -> Option<&dyn FieldDefaultProvider> {
        Some(self.defaults)
    }
}

impl FieldShape {
    /// Completes one absent field (or a deliberately skipped wire field).
    /// Explicit defaults take precedence over the ordinary missing Option rule.
    pub fn missing_value(
        &self,
        request: FieldDefaultRequest<'_>,
        access: &dyn ShapeAccess,
    ) -> Result<Value> {
        let shape = self.resolve_value_shape(access)?;
        if !self.has_default && !self.skip {
            return if matches!(shape.as_ref(), TypeShape::Option(_)) {
                Ok(Value::Option(None))
            } else {
                Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("missing record field `{}`", self.wire_name),
                ))
            };
        }
        let provider = access.field_defaults().ok_or_else(|| {
            DataError::new(
                DataErrorKind::MissingField,
                format!(
                    "field `{}` requires an admitted default producer",
                    self.wire_name
                ),
            )
        })?;
        let value = provider.default_value(request)?;
        crate::encode_with_shape_ref(&value, ShapeRef::Inline(shape.as_ref()), access)?;
        Ok(value)
    }
}
