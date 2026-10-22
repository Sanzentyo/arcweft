//! Borrowed, shape-directed datum encoding. The decoder's owned Avro values are
//! never constructed on this path, including for arrays of zero-byte nulls.

use apache_avro::Schema;
use arcweft_data::{
    DataError, DataErrorKind, FieldShape, Number, RecordPolicy, Result, ShapeAccess, ShapeRef,
    TypeShape, Value,
};

use crate::{
    codec::{option_schema, schema_mismatch, shape_is_option},
    container::{AvroDatum, DatumWriter},
    enum_value::variant_payload_field,
};

#[derive(Clone, Copy)]
pub(super) struct DataDatum<'a> {
    value: &'a Value,
    shape: ShapeRef<'a>,
    access: &'a dyn ShapeAccess,
    index: Option<usize>,
}

impl<'a> DataDatum<'a> {
    pub(super) fn new(value: &'a Value, shape: ShapeRef<'a>, access: &'a dyn ShapeAccess) -> Self {
        Self {
            value,
            shape,
            access,
            index: None,
        }
    }

    pub(super) fn at_index(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }

    fn write(&self, writer: DatumWriter<'_>) -> Result<()> {
        let shape = self.shape.resolve(self.access)?;
        match (shape.as_ref(), self.value) {
            (TypeShape::Ref(id), _) => {
                DataDatum::new(self.value, ShapeRef::Id(*id), self.access).write(writer)
            }
            (TypeShape::Option(inner), Value::Option(value)) => {
                let (null, some, _) = option_schema(writer.schema())?;
                match value {
                    None => writer.union(null, |writer| writer.null()),
                    Some(value) => writer.union(some, |writer| {
                        DataDatum::new(value, ShapeRef::Inline(inner), self.access).write(writer)
                    }),
                }
            }
            (TypeShape::Unit, Value::Unit) => writer.null(),
            (TypeShape::Bool, Value::Bool(value)) => writer.boolean(*value),
            (TypeShape::String, Value::String(value)) => writer.string(value),
            (TypeShape::Char, Value::Char(value)) => writer.string(value.encode_utf8(&mut [0; 4])),
            (TypeShape::Bytes { .. }, Value::Bytes(value)) => writer.bytes(value.as_slice()),
            (TypeShape::Seq(item), Value::Seq(values)) => writer.array(
                values
                    .iter()
                    .map(|value| DataDatum::new(value, ShapeRef::Inline(item), self.access)),
            ),
            (TypeShape::Record { fields, policy, .. }, Value::Record(values)) => {
                self.write_record(writer, values, fields, *policy)
            }
            (TypeShape::Enum { variants, .. }, Value::Enum { variant, payload }) => {
                let index = variants
                    .iter()
                    .position(|candidate| candidate.wire_name == *variant)
                    .ok_or_else(|| {
                        DataError::new(
                            DataErrorKind::InvalidEnumTag,
                            format!("unknown enum variant `{variant}`"),
                        )
                    })?;
                let variant = &variants[index];
                if matches!(writer.schema(), Schema::Enum(_)) {
                    if payload.is_some() {
                        return Err(DataError::invalid_type("unit enum", "payload enum"));
                    }
                    return writer.enumeration(&variant.wire_name);
                }
                writer
                    .union(index, |writer| {
                        let Schema::Record(record) = writer.schema() else {
                            return Err(schema_mismatch("Avro variant record", writer.schema()));
                        };
                        writer.record(|writer| match (&variant.payload, payload) {
                            (Some(shape), Some(value)) => {
                                let field = variant_payload_field(record, &variant.wire_name)?;
                                writer.field(&field.name, |writer| {
                                    DataDatum::new(value, ShapeRef::Inline(shape), self.access)
                                        .write(writer)
                                })
                            }
                            (None, None) => Ok(()),
                            (Some(_), None) => Err(DataError::new(
                                DataErrorKind::MissingField,
                                format!("missing enum payload for variant `{}`", variant.wire_name),
                            )),
                            (None, Some(_)) => {
                                Err(DataError::invalid_type("unit enum variant", "payload enum"))
                            }
                        })
                    })
                    .map_err(|error| error.at_variant(variant.wire_name.clone()))
            }
            (
                TypeShape::I8
                | TypeShape::I16
                | TypeShape::I32
                | TypeShape::I64
                | TypeShape::Isize
                | TypeShape::U8
                | TypeShape::U16
                | TypeShape::U32,
                Value::Number(number),
            ) => writer.integer(integer(number, shape.as_ref())?),
            (TypeShape::F32, Value::Number(Number::F32(value))) => writer.float(*value),
            (TypeShape::F64, Value::Number(Number::F64(value))) => writer.double(*value),
            _ => Err(DataError::invalid_type(
                shape.type_name(),
                self.value.type_name(),
            )),
        }
    }

    fn write_record(
        &self,
        writer: DatumWriter<'_>,
        values: &std::collections::BTreeMap<String, Value>,
        fields: &[FieldShape],
        policy: RecordPolicy,
    ) -> Result<()> {
        let Schema::Record(schema) = writer.schema() else {
            return Err(schema_mismatch("Avro record", writer.schema()));
        };
        if policy.deny_unknown_fields
            && let Some(name) = values.keys().find(|name| {
                !fields
                    .iter()
                    .any(|field| !field.skip && field.wire_name == **name)
            })
        {
            return Err(DataError::new(
                DataErrorKind::UnknownField,
                format!("unknown Avro field `{name}`"),
            )
            .at_field(name.clone()));
        }
        // Missing shape fields were previously rejected by the owned datum
        // conversion too. Defaults belong to decoding, not writer-schema repair.
        for field in fields.iter().filter(|field| !field.skip) {
            if !schema
                .fields
                .iter()
                .any(|candidate| candidate.name == field.wire_name)
            {
                return Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("missing Avro schema field `{}`", field.wire_name),
                )
                .at_field(field.wire_name.clone()));
            }
        }
        writer.record(|writer| {
            schema.fields.iter().try_for_each(|avro_field| {
                let field = fields
                    .iter()
                    .find(|field| !field.skip && field.wire_name == avro_field.name)
                    .ok_or_else(|| {
                        DataError::new(
                            DataErrorKind::MissingField,
                            format!("missing Avro field `{}`", avro_field.name),
                        )
                        .at_field(avro_field.name.clone())
                    })?;
                let shape = field
                    .resolve_value_shape(self.access)
                    .map_err(|error| error.at_field(field.wire_name.clone()))?;
                writer.field(&field.wire_name, |writer| {
                    match values.get(&field.wire_name) {
                        Some(value) => {
                            DataDatum::new(value, ShapeRef::Inline(shape.as_ref()), self.access)
                                .write(writer)
                        }
                        None if shape_is_option(ShapeRef::Inline(shape.as_ref()), self.access)? => {
                            let (null, _, _) = option_schema(writer.schema())?;
                            writer.union(null, |writer| writer.null())
                        }
                        None => Err(DataError::new(
                            DataErrorKind::MissingField,
                            format!("missing Avro field `{}`", field.wire_name),
                        )),
                    }
                })
            })
        })
    }
}

impl AvroDatum for DataDatum<'_> {
    fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
        self.write(writer).map_err(|error| match self.index {
            Some(index) => error.at_index(index),
            None => error,
        })
    }
}

fn integer(number: &Number, shape: &TypeShape) -> Result<i64> {
    let value = match number {
        Number::I(value) if shape.signed_bounds().is_some() => {
            let (minimum, maximum) = shape.signed_bounds().expect("signed shape checked");
            if *value < minimum || *value > maximum {
                return Err(DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    format!("number is out of range for {}", shape.type_name()),
                ));
            }
            *value
        }
        Number::U(value) if shape.unsigned_max().is_some() => {
            if *value > shape.unsigned_max().expect("unsigned shape checked") {
                return Err(DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    format!("number is out of range for {}", shape.type_name()),
                ));
            }
            i128::try_from(*value).map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "Avro long is i64-limited for unsigned values",
                )
            })?
        }
        _ => {
            return Err(DataError::invalid_type(
                shape.type_name(),
                number.type_name(),
            ));
        }
    };
    i64::try_from(value)
        .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "Avro long is i64-limited"))
}
