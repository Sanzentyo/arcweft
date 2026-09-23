use std::collections::{BTreeMap, BTreeSet};

use apache_avro::schema::{RecordField, Schema};
use apache_avro::types::Value as AvroValue;
use apache_avro::{Reader, Writer};
use arcweft_data::{
    Bytes, Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions, EncodeOptions, FieldShape,
    FormatId, Number, RecordPolicy, Result, ShapeAccess, ShapeRef, TypeShape, Value,
};

use crate::avro_preflight::{AvroTopLevel, preflight_avro_container};
use crate::enum_value;

#[derive(Clone, Debug)]
pub struct AvroCodec {
    schema: Schema,
}

impl AvroCodec {
    pub fn new(schema_json: &str) -> Result<Self> {
        Schema::parse_str(schema_json)
            .map(|schema| Self { schema })
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    }
}

impl Codec for AvroCodec {
    fn id(&self) -> FormatId {
        FormatId::new("avro")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/avro", "application/vnd.apache.avro"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["avro"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let resolved_shape = shape.resolve(access)?;
        match resolved_shape.as_ref() {
            TypeShape::Ref(id) => self.encode_value(value, ShapeRef::Id(*id), access, options),
            TypeShape::Seq(item_shape) => {
                validate_schema(ShapeRef::Inline(item_shape), access, &self.schema)?;
                let rows = value.as_seq()?;
                let mut writer = Writer::new(&self.schema, Vec::new());
                rows.iter().enumerate().try_for_each(|(index, value)| {
                    writer
                        .append(value_to_avro(
                            value,
                            ShapeRef::Inline(item_shape),
                            &self.schema,
                            access,
                        )?)
                        .map(|_| ())
                        .map_err(|error| {
                            DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                                .at_index(index)
                        })
                })?;
                writer.into_inner().map_err(invalid_encoding_error)
            }
            other => {
                validate_schema(ShapeRef::Inline(other), access, &self.schema)?;
                let mut writer = Writer::new(&self.schema, Vec::new());
                writer
                    .append(value_to_avro(
                        value,
                        ShapeRef::Inline(other),
                        &self.schema,
                        access,
                    )?)
                    .map(|_| ())
                    .map_err(invalid_encoding_error)?;
                writer.into_inner().map_err(invalid_encoding_error)
            }
        }
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let resolved_shape = shape.resolve(access)?;
        match resolved_shape.as_ref() {
            TypeShape::Ref(id) => self.decode_value(input, ShapeRef::Id(*id), access, options),
            TypeShape::Seq(item_shape) => {
                validate_schema(ShapeRef::Inline(item_shape), access, &self.schema)?;
                preflight_avro_container(input, &options.limits, AvroTopLevel::Sequence)?;
                let reader = Reader::new(input).map_err(invalid_encoding_error)?;
                budget.enter_node()?;
                let rows = reader
                    .enumerate()
                    .map(|(index, value)| {
                        budget.sequence_item(index.saturating_add(1))?;
                        value
                            .map_err(|error| {
                                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                                    .at_index(index)
                            })
                            .and_then(|value| {
                                avro_to_value(
                                    &value,
                                    ShapeRef::Inline(item_shape),
                                    &self.schema,
                                    &mut budget,
                                    access,
                                )
                                .map_err(|error| error.at_index(index))
                            })
                    })
                    .collect::<Result<Vec<_>>>();
                budget.exit_node();
                let rows = rows?;
                let value = Value::Seq(rows);
                options.limits.validate(&value)?;
                Ok(value)
            }
            other => {
                validate_schema(ShapeRef::Inline(other), access, &self.schema)?;
                preflight_avro_container(input, &options.limits, AvroTopLevel::Scalar)?;
                let mut reader = Reader::new(input).map_err(invalid_encoding_error)?;
                let Some(value) = reader.next() else {
                    return Err(DataError::new(
                        DataErrorKind::InvalidEncoding,
                        "expected exactly one Avro datum, found 0",
                    ));
                };
                let value = value.map_err(invalid_encoding_error)?;
                if reader
                    .next()
                    .transpose()
                    .map_err(invalid_encoding_error)?
                    .is_some()
                {
                    return Err(DataError::new(
                        DataErrorKind::InvalidEncoding,
                        "expected exactly one Avro datum, found more than one",
                    ));
                }
                let value = avro_to_value(&value, shape, &self.schema, &mut budget, access)?;
                options.limits.validate(&value)?;
                Ok(value)
            }
        }
    }
}

fn invalid_encoding_error(error: impl std::fmt::Display) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
}

pub(super) fn validate_schema(
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
    schema: &Schema,
) -> Result<()> {
    let resolved_shape = shape.resolve(access)?;
    match resolved_shape.as_ref() {
        TypeShape::Ref(id) => validate_schema(ShapeRef::Id(*id), access, schema),
        TypeShape::Option(inner)
            if shape_is_unit(ShapeRef::Inline(inner), access)?
                || shape_is_option(ShapeRef::Inline(inner), access)? =>
        {
            Err(DataError::unsupported(
                "Avro union nullability cannot distinguish this option shape",
            ))
        }
        TypeShape::Option(inner) => {
            let (_, _, inner_schema) = option_schema(schema)?;
            validate_schema(ShapeRef::Inline(inner), access, inner_schema)
        }
        TypeShape::Unit => expect_schema(schema, matches!(schema, Schema::Null), "Avro null"),
        TypeShape::Bool => expect_schema(schema, matches!(schema, Schema::Boolean), "Avro boolean"),
        TypeShape::I8 | TypeShape::I16 | TypeShape::I32 => expect_schema(
            schema,
            matches!(schema, Schema::Int | Schema::Long),
            "Avro int or long",
        ),
        TypeShape::I64 | TypeShape::Isize => {
            expect_schema(schema, matches!(schema, Schema::Long), "Avro long")
        }
        TypeShape::U8 | TypeShape::U16 => expect_schema(
            schema,
            matches!(schema, Schema::Int | Schema::Long),
            "Avro int or long",
        ),
        TypeShape::U32 => expect_schema(schema, matches!(schema, Schema::Long), "Avro long"),
        TypeShape::F32 => expect_schema(schema, matches!(schema, Schema::Float), "Avro float"),
        TypeShape::F64 => expect_schema(schema, matches!(schema, Schema::Double), "Avro double"),
        TypeShape::String | TypeShape::Char => {
            expect_schema(schema, matches!(schema, Schema::String), "Avro string")
        }
        TypeShape::Bytes { .. } => expect_schema(
            schema,
            matches!(schema, Schema::Bytes | Schema::Fixed(_)),
            "Avro bytes or fixed",
        ),
        TypeShape::Seq(item_shape) => {
            let Schema::Array(array) = schema else {
                return Err(schema_mismatch("Avro array", schema));
            };
            validate_schema(ShapeRef::Inline(item_shape), access, &array.items)
        }
        TypeShape::Tuple(_) => Err(DataError::unsupported(
            "Avro does not provide a positional heterogeneous tuple type",
        )),
        TypeShape::Map { .. } => Err(DataError::unsupported(
            "Avro maps do not preserve Arcweft map entry order",
        )),
        TypeShape::Record { fields, policy, .. } => {
            validate_record_schema(fields, *policy, access, schema)
        }
        TypeShape::Enum {
            variants,
            tag,
            repr,
            ..
        } => enum_value::validate_enum_schema(variants, tag, repr.as_ref(), access, schema),
        TypeShape::I128 | TypeShape::U64 | TypeShape::U128 | TypeShape::Usize => {
            Err(DataError::unsupported(format!(
                "Avro cannot represent the full {} range",
                resolved_shape.type_name()
            )))
        }
    }
}

fn shape_is_unit(shape: ShapeRef<'_>, access: &dyn ShapeAccess) -> Result<bool> {
    let resolved = shape.resolve(access)?;
    match resolved.as_ref() {
        TypeShape::Ref(id) => shape_is_unit(ShapeRef::Id(*id), access),
        TypeShape::Unit => Ok(true),
        _ => Ok(false),
    }
}

fn shape_is_option(shape: ShapeRef<'_>, access: &dyn ShapeAccess) -> Result<bool> {
    let resolved = shape.resolve(access)?;
    match resolved.as_ref() {
        TypeShape::Ref(id) => shape_is_option(ShapeRef::Id(*id), access),
        TypeShape::Option(_) => Ok(true),
        _ => Ok(false),
    }
}

fn validate_record_schema(
    fields: &[FieldShape],
    policy: RecordPolicy,
    access: &dyn ShapeAccess,
    schema: &Schema,
) -> Result<()> {
    let Schema::Record(record) = schema else {
        return Err(schema_mismatch("Avro record", schema));
    };
    let avro_fields = record
        .fields
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect::<BTreeMap<_, _>>();
    let shape_fields = fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| field.wire_name.as_str())
        .collect::<BTreeSet<_>>();

    if policy.deny_unknown_fields
        && let Some(name) = avro_fields
            .keys()
            .find(|name| !shape_fields.contains(**name))
    {
        return Err(DataError::new(
            DataErrorKind::UnknownField,
            format!("unknown Avro schema field `{name}`"),
        )
        .at_field(*name));
    }

    fields
        .iter()
        .filter(|field| !field.skip)
        .try_for_each(|field| {
            let shape = field
                .resolve_value_shape(access)
                .map_err(|error| error.at_field(field.wire_name.clone()))?;
            let Some(avro_field) = avro_fields.get(field.wire_name.as_str()) else {
                return if field.has_default || matches!(shape.as_ref(), TypeShape::Option(_)) {
                    Ok(())
                } else {
                    Err(DataError::new(
                        DataErrorKind::MissingField,
                        format!("missing Avro schema field `{}`", field.wire_name),
                    )
                    .at_field(field.wire_name.clone()))
                };
            };
            validate_schema(ShapeRef::Inline(shape.as_ref()), access, &avro_field.schema)
                .map_err(|error| error.at_field(field.wire_name.clone()))
        })
}

fn expect_schema(schema: &Schema, matches: bool, expected: &str) -> Result<()> {
    if matches {
        Ok(())
    } else {
        Err(schema_mismatch(expected, schema))
    }
}

pub(super) fn schema_mismatch(expected: &str, schema: &Schema) -> DataError {
    DataError::invalid_type(expected, schema_label(schema))
}

fn schema_label(schema: &Schema) -> &'static str {
    match schema {
        Schema::Null => "Avro null",
        Schema::Boolean => "Avro boolean",
        Schema::Int => "Avro int",
        Schema::Long => "Avro long",
        Schema::Float => "Avro float",
        Schema::Double => "Avro double",
        Schema::Bytes => "Avro bytes",
        Schema::String => "Avro string",
        Schema::Array(_) => "Avro array",
        Schema::Map(_) => "Avro map",
        Schema::Union(_) => "Avro union",
        Schema::Record(_) => "Avro record",
        Schema::Enum(_) => "Avro enum",
        Schema::Fixed(_) => "Avro fixed",
        Schema::Decimal(_) | Schema::BigDecimal => "Avro decimal",
        Schema::Uuid => "Avro uuid",
        Schema::Date => "Avro date",
        Schema::TimeMillis => "Avro time-millis",
        Schema::TimeMicros => "Avro time-micros",
        Schema::TimestampMillis => "Avro timestamp-millis",
        Schema::TimestampMicros => "Avro timestamp-micros",
        Schema::TimestampNanos => "Avro timestamp-nanos",
        Schema::LocalTimestampMillis => "Avro local-timestamp-millis",
        Schema::LocalTimestampMicros => "Avro local-timestamp-micros",
        Schema::LocalTimestampNanos => "Avro local-timestamp-nanos",
        Schema::Duration => "Avro duration",
        Schema::Ref { .. } => "Avro ref",
    }
}

fn option_schema(schema: &Schema) -> Result<(usize, usize, &Schema)> {
    let Schema::Union(union) = schema else {
        return Err(schema_mismatch("Avro union containing null", schema));
    };
    let variants = union.variants();
    let null_index = variants
        .iter()
        .position(|schema| matches!(schema, Schema::Null))
        .ok_or_else(|| schema_mismatch("Avro union containing null", schema))?;
    let non_null = variants
        .iter()
        .enumerate()
        .filter(|(_, schema)| !matches!(schema, Schema::Null))
        .collect::<Vec<_>>();
    let [(inner_index, inner_schema)] = non_null.as_slice() else {
        return Err(DataError::unsupported(
            "Arcweft option requires an Avro union with exactly one non-null branch",
        ));
    };
    Ok((null_index, *inner_index, inner_schema))
}

pub(super) fn value_to_avro(
    value: &Value,
    shape_ref: ShapeRef<'_>,
    schema: &Schema,
    access: &dyn ShapeAccess,
) -> Result<AvroValue> {
    let shape = shape_ref.resolve(access)?;
    match shape.as_ref() {
        TypeShape::Ref(id) => value_to_avro(value, ShapeRef::Id(*id), schema, access),
        TypeShape::Option(inner) => {
            let (null_index, value_index, inner_schema) = option_schema(schema)?;
            match value {
                Value::Option(None) => Ok(AvroValue::Union(
                    u32::try_from(null_index).expect("union index fits u32"),
                    Box::new(AvroValue::Null),
                )),
                Value::Option(Some(value)) => {
                    value_to_avro(value, ShapeRef::Inline(inner), inner_schema, access).map(
                        |value| {
                            AvroValue::Union(
                                u32::try_from(value_index).expect("union index fits u32"),
                                Box::new(value),
                            )
                        },
                    )
                }
                other => Err(DataError::invalid_type("option", other.type_name())),
            }
        }
        TypeShape::Unit
        | TypeShape::Bool
        | TypeShape::String
        | TypeShape::Char
        | TypeShape::Bytes { .. } => value_to_avro_scalar(value, shape.as_ref(), schema),
        TypeShape::Seq(item_shape) => {
            value_to_avro_array(value, ShapeRef::Inline(item_shape), schema, access)
        }
        TypeShape::Tuple(_) => Err(DataError::unsupported(
            "Avro does not provide a positional heterogeneous tuple type",
        )),
        TypeShape::Map { .. } => Err(DataError::unsupported(
            "Avro maps do not preserve Arcweft map entry order",
        )),
        TypeShape::Record { fields, policy, .. } => {
            value_to_avro_record(value, fields, *policy, schema, access)
        }
        TypeShape::Enum { variants, .. } => {
            enum_value::value_to_avro_enum(value, variants, schema, access)
        }
        TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::Isize
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32 => value_to_avro_integer(value, shape.as_ref(), schema),
        TypeShape::F32 | TypeShape::F64 => value_to_avro_float(value, shape.as_ref()),
        unsupported => Err(DataError::unsupported(format!(
            "Avro shape {} is not supported",
            unsupported.type_name()
        ))),
    }
}

fn value_to_avro_scalar(value: &Value, shape: &TypeShape, schema: &Schema) -> Result<AvroValue> {
    match shape {
        TypeShape::Unit => match value {
            Value::Unit => Ok(AvroValue::Null),
            other => Err(DataError::invalid_type("unit", other.type_name())),
        },
        TypeShape::Bool => match value {
            Value::Bool(value) => Ok(AvroValue::Boolean(*value)),
            other => Err(DataError::invalid_type("bool", other.type_name())),
        },
        TypeShape::String => match value {
            Value::String(value) => Ok(AvroValue::String(value.clone())),
            other => Err(DataError::invalid_type("string", other.type_name())),
        },
        TypeShape::Char => match value {
            Value::Char(value) => Ok(AvroValue::String(value.to_string())),
            other => Err(DataError::invalid_type("char", other.type_name())),
        },
        TypeShape::Bytes { .. } => value_to_avro_bytes(value, schema),
        other => Err(DataError::unsupported(format!(
            "Avro scalar shape {} is not supported",
            other.type_name()
        ))),
    }
}

fn value_to_avro_bytes(value: &Value, schema: &Schema) -> Result<AvroValue> {
    let Value::Bytes(bytes) = value else {
        return Err(DataError::invalid_type("bytes", value.type_name()));
    };
    match schema {
        Schema::Fixed(fixed) if bytes.as_slice().len() == fixed.size => {
            Ok(AvroValue::Fixed(fixed.size, bytes.as_slice().to_vec()))
        }
        Schema::Fixed(fixed) => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "fixed Avro field requires {} bytes, found {}",
                fixed.size,
                bytes.as_slice().len()
            ),
        )),
        _ => Ok(AvroValue::Bytes(bytes.as_slice().to_vec())),
    }
}

fn value_to_avro_array(
    value: &Value,
    item_shape: ShapeRef<'_>,
    schema: &Schema,
    access: &dyn ShapeAccess,
) -> Result<AvroValue> {
    let Schema::Array(array) = schema else {
        return Err(schema_mismatch("Avro array", schema));
    };
    let values = value.as_seq()?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value_to_avro(value, item_shape, &array.items, access)
                .map_err(|error| error.at_index(index))
        })
        .collect::<Result<Vec<_>>>()
        .map(AvroValue::Array)
}

fn value_to_avro_record(
    value: &Value,
    fields: &[FieldShape],
    policy: RecordPolicy,
    schema: &Schema,
    access: &dyn ShapeAccess,
) -> Result<AvroValue> {
    let Schema::Record(record_schema) = schema else {
        return Err(schema_mismatch("Avro record", schema));
    };
    let record = value.as_record()?;
    reject_unknown_fields(record.keys(), fields, policy)?;
    fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| {
            let avro_field = avro_record_field(record_schema, &field.wire_name)?;
            let shape = field
                .resolve_value_shape(access)
                .map_err(|error| error.at_field(field.wire_name.clone()))?;
            match record.get(&field.wire_name) {
                Some(value) => value_to_avro(
                    value,
                    ShapeRef::Inline(shape.as_ref()),
                    &avro_field.schema,
                    access,
                ),
                None if shape_is_option(ShapeRef::Inline(shape.as_ref()), access)? => {
                    value_to_avro(
                        &Value::Option(None),
                        ShapeRef::Inline(shape.as_ref()),
                        &avro_field.schema,
                        access,
                    )
                }
                None => Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("missing Avro field `{}`", field.wire_name),
                )),
            }
            .map(|value| (field.wire_name.clone(), value))
            .map_err(|error| error.at_field(field.wire_name.clone()))
        })
        .collect::<Result<Vec<_>>>()
        .map(AvroValue::Record)
}

fn avro_record_field<'a>(
    record: &'a apache_avro::schema::RecordSchema,
    name: &str,
) -> Result<&'a RecordField> {
    record
        .fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::MissingField,
                format!("missing Avro schema field `{name}`"),
            )
            .at_field(name.to_owned())
        })
}

fn reject_unknown_fields<'a>(
    names: impl Iterator<Item = &'a String>,
    fields: &[FieldShape],
    policy: RecordPolicy,
) -> Result<()> {
    if !policy.deny_unknown_fields {
        return Ok(());
    }
    let known = fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| field.wire_name.as_str())
        .collect::<BTreeSet<_>>();
    names
        .filter(|name| !known.contains(name.as_str()))
        .map(|name| {
            DataError::new(
                DataErrorKind::UnknownField,
                format!("unknown Avro field `{name}`"),
            )
            .at_field(name.clone())
        })
        .next()
        .map_or(Ok(()), Err)
}

fn value_to_avro_integer(value: &Value, shape: &TypeShape, schema: &Schema) -> Result<AvroValue> {
    let Value::Number(number) = value else {
        return Err(DataError::invalid_type("number", value.type_name()));
    };
    let integer = match number {
        Number::I(value) if shape.signed_bounds().is_some() => {
            let (min, max) = shape.signed_bounds().expect("signed shape checked above");
            if *value < min || *value > max {
                return Err(DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    format!("number is out of range for {}", shape.type_name()),
                ));
            }
            *value
        }
        Number::U(value) if shape.unsigned_max().is_some() => {
            let max = shape
                .unsigned_max()
                .expect("unsigned shape checked by caller");
            if *value > max {
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
        Number::I(_) | Number::U(_) => {
            return Err(DataError::invalid_type(
                shape.type_name(),
                number.type_name(),
            ));
        }
        Number::F32(_) | Number::F64(_) => {
            return Err(DataError::invalid_type("integer", number.type_name()));
        }
    };
    match schema {
        Schema::Int => i32::try_from(integer).map(AvroValue::Int).map_err(|_| {
            DataError::new(DataErrorKind::NumberOutOfRange, "Avro int is i32-limited")
        }),
        Schema::Long => i64::try_from(integer).map(AvroValue::Long).map_err(|_| {
            DataError::new(DataErrorKind::NumberOutOfRange, "Avro long is i64-limited")
        }),
        other => Err(schema_mismatch("Avro int or long", other)),
    }
}

fn value_to_avro_float(value: &Value, shape: &TypeShape) -> Result<AvroValue> {
    let Value::Number(number) = value else {
        return Err(DataError::invalid_type("number", value.type_name()));
    };
    match (shape, number) {
        (TypeShape::F32, Number::F32(value)) if value.is_finite() => Ok(AvroValue::Float(*value)),
        (TypeShape::F64, Number::F64(value)) if value.is_finite() => Ok(AvroValue::Double(*value)),
        (TypeShape::F32 | TypeShape::F64, Number::F32(_) | Number::F64(_)) => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "Avro floats must be finite",
        )),
        _ => Err(DataError::invalid_type(
            shape.type_name(),
            number.type_name(),
        )),
    }
}

pub(super) fn avro_to_value(
    value: &AvroValue,
    shape: ShapeRef<'_>,
    schema: &Schema,
    budget: &mut DecodeBudget<'_>,
    access: &dyn ShapeAccess,
) -> Result<Value> {
    budget.enter_node()?;
    let result = avro_to_value_inner(value, shape, schema, budget, access);
    budget.exit_node();
    result
}

fn avro_to_value_inner(
    value: &AvroValue,
    shape_ref: ShapeRef<'_>,
    schema: &Schema,
    budget: &mut DecodeBudget<'_>,
    access: &dyn ShapeAccess,
) -> Result<Value> {
    let shape = shape_ref.resolve(access)?;
    match shape.as_ref() {
        TypeShape::Ref(id) => avro_to_value(value, ShapeRef::Id(*id), schema, budget, access),
        TypeShape::Option(inner) => {
            let (_, _, inner_schema) = option_schema(schema)?;
            match value {
                AvroValue::Null => Ok(Value::Option(None)),
                AvroValue::Union(_, inner_value)
                    if matches!(inner_value.as_ref(), AvroValue::Null) =>
                {
                    Ok(Value::Option(None))
                }
                AvroValue::Union(_, inner_value) => avro_to_value(
                    inner_value,
                    ShapeRef::Inline(inner),
                    inner_schema,
                    budget,
                    access,
                )
                .map(Box::new)
                .map(Some)
                .map(Value::Option),
                other => {
                    avro_to_value(other, ShapeRef::Inline(inner), inner_schema, budget, access)
                        .map(Box::new)
                        .map(Some)
                        .map(Value::Option)
                }
            }
        }
        TypeShape::Unit
        | TypeShape::Bool
        | TypeShape::String
        | TypeShape::Char
        | TypeShape::Bytes { .. } => avro_to_value_scalar(value, shape.as_ref(), budget),
        TypeShape::Seq(item_shape) => {
            avro_to_value_array(value, ShapeRef::Inline(item_shape), schema, budget, access)
        }
        TypeShape::Tuple(_) => Err(DataError::unsupported(
            "Avro does not provide a positional heterogeneous tuple type",
        )),
        TypeShape::Map { .. } => Err(DataError::unsupported(
            "Avro maps do not preserve Arcweft map entry order",
        )),
        TypeShape::Record { fields, policy, .. } => {
            avro_to_value_record(value, shape_ref, fields, *policy, schema, budget, access)
        }
        TypeShape::Enum { variants, .. } => {
            enum_value::avro_to_value_enum(value, variants, schema, budget, access)
        }
        TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::Isize
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32 => avro_to_value_integer(value, shape.as_ref()),
        TypeShape::F32 | TypeShape::F64 => avro_to_value_float(value, shape.as_ref()),
        unsupported => Err(DataError::unsupported(format!(
            "Avro shape {} is not supported",
            unsupported.type_name()
        ))),
    }
}

fn avro_to_value_scalar(
    value: &AvroValue,
    shape: &TypeShape,
    budget: &DecodeBudget<'_>,
) -> Result<Value> {
    match shape {
        TypeShape::Unit => match value {
            AvroValue::Null => Ok(Value::Unit),
            other => Err(DataError::invalid_type("unit", avro_value_label(other))),
        },
        TypeShape::Bool => match value {
            AvroValue::Boolean(value) => Ok(Value::Bool(*value)),
            other => Err(DataError::invalid_type("bool", avro_value_label(other))),
        },
        TypeShape::String => match value {
            AvroValue::String(value) => {
                budget.string_len(value.len())?;
                Ok(Value::String(value.clone()))
            }
            other => Err(DataError::invalid_type("string", avro_value_label(other))),
        },
        TypeShape::Char => match value {
            AvroValue::String(value) => {
                budget.string_len(value.len())?;
                parse_char(value)
            }
            other => Err(DataError::invalid_type("char", avro_value_label(other))),
        },
        TypeShape::Bytes { .. } => match value {
            AvroValue::Bytes(value) | AvroValue::Fixed(_, value) => {
                budget.bytes_len(value.len())?;
                Ok(Value::Bytes(Bytes::new(value.clone())))
            }
            other => Err(DataError::invalid_type("bytes", avro_value_label(other))),
        },
        other => Err(DataError::unsupported(format!(
            "Avro scalar shape {} is not supported",
            other.type_name()
        ))),
    }
}

fn avro_to_value_array(
    value: &AvroValue,
    item_shape: ShapeRef<'_>,
    schema: &Schema,
    budget: &mut DecodeBudget<'_>,
    access: &dyn ShapeAccess,
) -> Result<Value> {
    let Schema::Array(array) = schema else {
        return Err(schema_mismatch("Avro array", schema));
    };
    let AvroValue::Array(values) = value else {
        return Err(DataError::invalid_type("array", avro_value_label(value)));
    };
    budget.sequence_len(values.len())?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            avro_to_value(value, item_shape, &array.items, budget, access)
                .map_err(|error| error.at_index(index))
        })
        .collect::<Result<Vec<_>>>()
        .map(Value::Seq)
}

fn avro_to_value_record(
    value: &AvroValue,
    record_shape: ShapeRef<'_>,
    fields: &[FieldShape],
    policy: RecordPolicy,
    schema: &Schema,
    budget: &mut DecodeBudget<'_>,
    access: &dyn ShapeAccess,
) -> Result<Value> {
    let Schema::Record(record_schema) = schema else {
        return Err(schema_mismatch("Avro record", schema));
    };
    let AvroValue::Record(values) = value else {
        return Err(DataError::invalid_type("record", avro_value_label(value)));
    };
    let values = values
        .iter()
        .map(|(key, value)| (key.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    reject_unknown_value_fields(values.keys().copied(), fields, policy)?;
    budget.map_len(fields.len())?;
    fields
        .iter()
        .enumerate()
        .map(|(ordinal, field)| {
            match (!field.skip)
                .then(|| values.get(field.wire_name.as_str()))
                .flatten()
            {
                Some(value) => {
                    let avro_field = avro_record_field(record_schema, &field.wire_name)?;
                    // Avro bytes are already normalized; retain the declaration
                    // edge so nested defaults keep their original coordinate.
                    avro_to_value(
                        value,
                        ShapeRef::Inline(&field.shape),
                        &avro_field.schema,
                        budget,
                        access,
                    )
                }
                None => field.missing_value(
                    arcweft_data::FieldDefaultRequest::new(record_shape, ordinal),
                    access,
                ),
            }
            .map(|value| (field.wire_name.clone(), value))
            .map_err(|error| error.at_field(field.wire_name.clone()))
        })
        .collect::<Result<BTreeMap<_, _>>>()
        .map(Value::Record)
}

fn reject_unknown_value_fields<'a>(
    names: impl Iterator<Item = &'a str>,
    fields: &[FieldShape],
    policy: RecordPolicy,
) -> Result<()> {
    if !policy.deny_unknown_fields {
        return Ok(());
    }
    let known = fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| field.wire_name.as_str())
        .collect::<BTreeSet<_>>();
    names
        .filter(|name| !known.contains(name))
        .map(|name| {
            DataError::new(
                DataErrorKind::UnknownField,
                format!("unknown Avro field `{name}`"),
            )
            .at_field(name.to_owned())
        })
        .next()
        .map_or(Ok(()), Err)
}

fn avro_to_value_integer(value: &AvroValue, shape: &TypeShape) -> Result<Value> {
    let integer = match value {
        AvroValue::Int(value) => i128::from(*value),
        AvroValue::Long(value) => i128::from(*value),
        other => return Err(DataError::invalid_type("integer", avro_value_label(other))),
    };
    if let Some((min, max)) = shape.signed_bounds() {
        if integer < min || integer > max {
            return Err(DataError::new(
                DataErrorKind::NumberOutOfRange,
                format!("number is out of range for {}", shape.type_name()),
            ));
        }
        return Ok(Value::Number(Number::I(integer)));
    }
    if integer < 0 {
        return Err(DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("negative number is out of range for {}", shape.type_name()),
        ));
    }
    let unsigned = u128::try_from(integer).expect("integer is non-negative");
    let max = shape
        .unsigned_max()
        .expect("unsigned shape checked by caller");
    if unsigned > max {
        return Err(DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("number is out of range for {}", shape.type_name()),
        ));
    }
    Ok(Value::Number(Number::U(unsigned)))
}

fn avro_to_value_float(value: &AvroValue, shape: &TypeShape) -> Result<Value> {
    match (shape, value) {
        (TypeShape::F32, AvroValue::Float(value)) if value.is_finite() => {
            Ok(Value::Number(Number::F32(*value)))
        }
        (TypeShape::F64, AvroValue::Double(value)) if value.is_finite() => {
            Ok(Value::Number(Number::F64(*value)))
        }
        (TypeShape::F32 | TypeShape::F64, AvroValue::Float(_) | AvroValue::Double(_)) => Err(
            DataError::new(DataErrorKind::InvalidEncoding, "Avro floats must be finite"),
        ),
        _ => Err(DataError::invalid_type(
            shape.type_name(),
            avro_value_label(value),
        )),
    }
}

fn parse_char(value: &str) -> Result<Value> {
    let mut chars = value.chars();
    let Some(ch) = chars.next() else {
        return Err(DataError::invalid_type("single char", "empty string"));
    };
    if chars.next().is_some() {
        return Err(DataError::invalid_type("single char", "multi-char string"));
    }
    Ok(Value::Char(ch))
}

pub(super) fn avro_value_label(value: &AvroValue) -> &'static str {
    match value {
        AvroValue::Null => "Avro null",
        AvroValue::Boolean(_) => "Avro boolean",
        AvroValue::Int(_) => "Avro int",
        AvroValue::Long(_) => "Avro long",
        AvroValue::Float(_) => "Avro float",
        AvroValue::Double(_) => "Avro double",
        AvroValue::Bytes(_) => "Avro bytes",
        AvroValue::String(_) => "Avro string",
        AvroValue::Fixed(_, _) => "Avro fixed",
        AvroValue::Enum(_, _) => "Avro enum",
        AvroValue::Union(_, _) => "Avro union",
        AvroValue::Array(_) => "Avro array",
        AvroValue::Map(_) => "Avro map",
        AvroValue::Record(_) => "Avro record",
        AvroValue::Date(_) => "Avro date",
        AvroValue::Decimal(_) | AvroValue::BigDecimal(_) => "Avro decimal",
        AvroValue::TimeMillis(_) => "Avro time-millis",
        AvroValue::TimeMicros(_) => "Avro time-micros",
        AvroValue::TimestampMillis(_) => "Avro timestamp-millis",
        AvroValue::TimestampMicros(_) => "Avro timestamp-micros",
        AvroValue::TimestampNanos(_) => "Avro timestamp-nanos",
        AvroValue::LocalTimestampMillis(_) => "Avro local-timestamp-millis",
        AvroValue::LocalTimestampMicros(_) => "Avro local-timestamp-micros",
        AvroValue::LocalTimestampNanos(_) => "Avro local-timestamp-nanos",
        AvroValue::Duration(_) => "Avro duration",
        AvroValue::Uuid(_) => "Avro uuid",
    }
}
