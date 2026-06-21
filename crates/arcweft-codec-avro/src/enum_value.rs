use apache_avro::schema::{RecordField, Schema};
use apache_avro::types::Value as AvroValue;
use arcweft_data::{
    DataError, DataErrorKind, DecodeBudget, EnumTagStyle, Result, Value, VariantShape,
};

use crate::codec::{
    avro_to_value, avro_value_label, schema_mismatch, validate_schema, value_to_avro,
};

pub(super) fn validate_enum_schema(
    variants: &[VariantShape],
    tag: &EnumTagStyle,
    repr: Option<&arcweft_data::EnumRepr>,
    schema: &Schema,
) -> Result<()> {
    if repr.is_some() {
        return Err(DataError::unsupported(
            "Avro enum values are symbolic; numeric repr enum shapes are not supported",
        ));
    }
    if !matches!(tag, EnumTagStyle::External) {
        return Err(DataError::unsupported(
            "Avro enum mapping requires external enum tags",
        ));
    }
    if variants.iter().all(|variant| variant.payload.is_none()) && matches!(schema, Schema::Enum(_))
    {
        return validate_native_enum_schema(variants, schema);
    }
    validate_payload_enum_schema(variants, schema)
}

fn validate_native_enum_schema(variants: &[VariantShape], schema: &Schema) -> Result<()> {
    let Schema::Enum(avro_enum) = schema else {
        return Err(schema_mismatch("Avro enum", schema));
    };
    let expected = variants
        .iter()
        .map(|variant| variant.wire_name.as_str())
        .collect::<Vec<_>>();
    let actual = avro_enum
        .symbols
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if expected == actual {
        Ok(())
    } else {
        Err(DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!("Avro enum symbols {actual:?} do not match Arcweft variants {expected:?}"),
        ))
    }
}

fn validate_payload_enum_schema(variants: &[VariantShape], schema: &Schema) -> Result<()> {
    let Schema::Union(union) = schema else {
        return Err(schema_mismatch("Avro union of variant records", schema));
    };
    let branches = union.variants();
    if branches.len() != variants.len() {
        return Err(DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!(
                "Avro enum union has {} branches, but Arcweft shape has {} variants",
                branches.len(),
                variants.len()
            ),
        ));
    }
    variants
        .iter()
        .zip(branches)
        .try_for_each(|(variant, schema)| {
            validate_variant_record_schema(variant, schema)
                .map_err(|error| error.at_variant(variant.wire_name.clone()))
        })
}

fn validate_variant_record_schema(variant: &VariantShape, schema: &Schema) -> Result<()> {
    let Schema::Record(record) = schema else {
        return Err(schema_mismatch("Avro variant record", schema));
    };
    if record.name.name != variant.wire_name {
        return Err(DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!(
                "Avro variant record `{}` does not match Arcweft variant `{}`",
                record.name.name, variant.wire_name
            ),
        ));
    }
    match &variant.payload {
        Some(payload_shape) => {
            let field = variant_payload_field(record, &variant.wire_name)?;
            validate_schema(payload_shape, &field.schema)
        }
        None if record.fields.is_empty() => Ok(()),
        None => Err(DataError::new(
            DataErrorKind::InvalidType,
            format!(
                "unit variant `{}` must use an empty Avro record branch",
                variant.wire_name
            ),
        )),
    }
}

pub(super) fn value_to_avro_enum(
    value: &Value,
    variants: &[VariantShape],
    schema: &Schema,
) -> Result<AvroValue> {
    if matches!(schema, Schema::Enum(_)) {
        return value_to_avro_native_enum(value, variants, schema);
    }
    value_to_avro_payload_enum(value, variants, schema)
}

fn value_to_avro_native_enum(
    value: &Value,
    variants: &[VariantShape],
    schema: &Schema,
) -> Result<AvroValue> {
    let Schema::Enum(avro_enum) = schema else {
        return Err(schema_mismatch("Avro enum", schema));
    };
    let Value::Enum { variant, payload } = value else {
        return Err(DataError::invalid_type("enum", value.type_name()));
    };
    if payload.is_some() {
        return Err(DataError::invalid_type("unit enum", "payload enum"));
    }
    let shape_index = variants
        .iter()
        .position(|candidate| candidate.wire_name == *variant)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("unknown enum variant `{variant}`"),
            )
        })?;
    let schema_index = avro_enum
        .symbols
        .iter()
        .position(|symbol| symbol == variant)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("Avro enum schema does not contain `{variant}`"),
            )
        })?;
    if shape_index != schema_index {
        return Err(DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!("enum variant `{variant}` has mismatched Avro index"),
        ));
    }
    Ok(AvroValue::Enum(
        u32::try_from(schema_index).expect("enum index fits u32"),
        variant.clone(),
    ))
}

fn value_to_avro_payload_enum(
    value: &Value,
    variants: &[VariantShape],
    schema: &Schema,
) -> Result<AvroValue> {
    let Schema::Union(union) = schema else {
        return Err(schema_mismatch("Avro union of variant records", schema));
    };
    let Value::Enum { variant, payload } = value else {
        return Err(DataError::invalid_type("enum", value.type_name()));
    };
    let index = variants
        .iter()
        .position(|candidate| candidate.wire_name == *variant)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("unknown enum variant `{variant}`"),
            )
        })?;
    let variant_shape = &variants[index];
    let branch_schema = union.variants().get(index).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!("missing Avro branch for enum variant `{variant}`"),
        )
    })?;
    let record = value_to_variant_record(variant_shape, payload.as_deref(), branch_schema)?;
    Ok(AvroValue::Union(
        u32::try_from(index).expect("enum union index fits u32"),
        Box::new(record),
    ))
}

fn value_to_variant_record(
    variant: &VariantShape,
    payload: Option<&Value>,
    schema: &Schema,
) -> Result<AvroValue> {
    let Schema::Record(record) = schema else {
        return Err(schema_mismatch("Avro variant record", schema));
    };
    if record.name.name != variant.wire_name {
        return Err(DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!(
                "Avro variant record `{}` does not match Arcweft variant `{}`",
                record.name.name, variant.wire_name
            ),
        ));
    }
    match (&variant.payload, payload) {
        (Some(payload_shape), Some(payload)) => {
            let field = variant_payload_field(record, &variant.wire_name)?;
            value_to_avro(payload, payload_shape, &field.schema)
                .map(|payload| AvroValue::Record(vec![("payload".to_owned(), payload)]))
                .map_err(|error| error.at_variant(variant.wire_name.clone()))
        }
        (Some(_), None) => Err(DataError::new(
            DataErrorKind::MissingField,
            format!("missing enum payload for variant `{}`", variant.wire_name),
        )
        .at_variant(variant.wire_name.clone())),
        (None, None) => Ok(AvroValue::Record(Vec::new())),
        (None, Some(_)) => Err(DataError::invalid_type("unit enum variant", "payload enum")
            .at_variant(variant.wire_name.clone())),
    }
}

fn variant_payload_field<'a>(
    record: &'a apache_avro::schema::RecordSchema,
    variant: &str,
) -> Result<&'a RecordField> {
    let [field] = record.fields.as_slice() else {
        return Err(DataError::new(
            DataErrorKind::InvalidType,
            format!("payload variant `{variant}` requires one Avro `payload` field"),
        ));
    };
    if field.name == "payload" {
        Ok(field)
    } else {
        Err(DataError::new(
            DataErrorKind::MissingField,
            format!("payload variant `{variant}` requires Avro field `payload`"),
        ))
    }
}

pub(super) fn avro_to_value_enum(
    value: &AvroValue,
    variants: &[VariantShape],
    schema: &Schema,
    budget: &mut DecodeBudget<'_>,
) -> Result<Value> {
    if matches!(schema, Schema::Enum(_)) {
        return avro_to_value_native_enum(value, variants, schema);
    }
    avro_to_value_payload_enum(value, variants, schema, budget)
}

fn avro_to_value_native_enum(
    value: &AvroValue,
    variants: &[VariantShape],
    schema: &Schema,
) -> Result<Value> {
    let Schema::Enum(avro_enum) = schema else {
        return Err(schema_mismatch("Avro enum", schema));
    };
    let AvroValue::Enum(index, variant) = value else {
        return Err(DataError::invalid_type("enum", avro_value_label(value)));
    };
    let schema_index = avro_enum
        .symbols
        .iter()
        .position(|symbol| symbol == variant)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("Avro enum schema does not contain `{variant}`"),
            )
        })?;
    let shape_index = variants
        .iter()
        .position(|candidate| candidate.wire_name == *variant)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("unknown enum variant `{variant}`"),
            )
        })?;
    if *index as usize != schema_index || schema_index != shape_index {
        return Err(DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!("enum variant `{variant}` has mismatched Avro index"),
        ));
    }
    Ok(Value::Enum {
        variant: variant.clone(),
        payload: None,
    })
}

fn avro_to_value_payload_enum(
    value: &AvroValue,
    variants: &[VariantShape],
    schema: &Schema,
    budget: &mut DecodeBudget<'_>,
) -> Result<Value> {
    let Schema::Union(union) = schema else {
        return Err(schema_mismatch("Avro union of variant records", schema));
    };
    let AvroValue::Union(index, inner) = value else {
        return Err(DataError::invalid_type(
            "Avro union",
            avro_value_label(value),
        ));
    };
    let index = usize::try_from(*index).expect("u32 union index fits usize");
    let variant = variants.get(index).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!("Avro enum union index {index} is outside Arcweft variant list"),
        )
    })?;
    let branch_schema = union.variants().get(index).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!("Avro enum union index {index} is outside schema branch list"),
        )
    })?;
    avro_variant_record_to_value(inner, variant, branch_schema, budget)
        .map_err(|error| error.at_variant(variant.wire_name.clone()))
}

fn avro_variant_record_to_value(
    value: &AvroValue,
    variant: &VariantShape,
    schema: &Schema,
    budget: &mut DecodeBudget<'_>,
) -> Result<Value> {
    let Schema::Record(record_schema) = schema else {
        return Err(schema_mismatch("Avro variant record", schema));
    };
    if record_schema.name.name != variant.wire_name {
        return Err(DataError::new(
            DataErrorKind::InvalidEnumTag,
            format!(
                "Avro variant record `{}` does not match Arcweft variant `{}`",
                record_schema.name.name, variant.wire_name
            ),
        ));
    }
    let AvroValue::Record(fields) = value else {
        return Err(DataError::invalid_type("record", avro_value_label(value)));
    };
    match &variant.payload {
        Some(payload_shape) => {
            let schema_field = variant_payload_field(record_schema, &variant.wire_name)?;
            let payload = fields
                .iter()
                .find(|(name, _)| name == "payload")
                .ok_or_else(|| {
                    DataError::new(
                        DataErrorKind::MissingField,
                        format!("missing Avro payload for variant `{}`", variant.wire_name),
                    )
                })?;
            let payload = avro_to_value(&payload.1, payload_shape, &schema_field.schema, budget)?;
            Ok(Value::Enum {
                variant: variant.wire_name.clone(),
                payload: Some(Box::new(payload)),
            })
        }
        None if fields.is_empty() => Ok(Value::Enum {
            variant: variant.wire_name.clone(),
            payload: None,
        }),
        None => Err(DataError::new(
            DataErrorKind::UnknownField,
            format!(
                "unit variant `{}` has Avro payload fields",
                variant.wire_name
            ),
        )),
    }
}
