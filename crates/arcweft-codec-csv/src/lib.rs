#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use arcweft_data::{
    Bytes, BytesFormat, Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions,
    EncodeOptions, FieldShape, FormatId, Number, RecordPolicy, Result, TypeShape, Value,
};
use base64::{
    decoded_len_estimate,
    prelude::{BASE64_STANDARD, Engine as _},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct CsvCodec;

impl Codec for CsvCodec {
    fn id(&self) -> FormatId {
        FormatId::new("csv")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["text/csv"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["csv"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let row_shape = csv_row_shape(shape)?;
        let rows = value.as_seq()?;
        let headers = csv_headers(row_shape.fields);
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer
            .write_record(&headers)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        rows.iter()
            .enumerate()
            .try_for_each(|(index, row)| write_row(&mut writer, row, row_shape, &headers, index))?;
        writer
            .into_inner()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let row_shape = csv_row_shape(shape)?;
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let mut reader = csv::Reader::from_reader(input);
        let headers = reader
            .headers()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?
            .iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        validate_headers(&headers, row_shape)?;
        let row_indexes = row_indexes(&headers, row_shape.fields);
        budget.enter_node()?;
        let rows = reader
            .records()
            .enumerate()
            .map(|(row_index, record)| {
                budget.sequence_item(row_index.saturating_add(1))?;
                let record = record.map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                        .at_index(row_index)
                })?;
                decode_row(
                    &record,
                    row_shape.fields,
                    &row_indexes,
                    row_index,
                    &mut budget,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        budget.exit_node();
        let value = Value::Seq(rows);
        options.limits.validate(&value)?;
        Ok(value)
    }
}

#[derive(Clone, Copy)]
struct CsvRowShape<'a> {
    fields: &'a [FieldShape],
    policy: RecordPolicy,
}

fn csv_row_shape(shape: &TypeShape) -> Result<CsvRowShape<'_>> {
    let TypeShape::Seq(row_shape) = shape else {
        return Err(DataError::unsupported(
            "CSV requires a top-level sequence of record rows",
        ));
    };
    let TypeShape::Record { fields, policy, .. } = row_shape.as_ref() else {
        return Err(DataError::unsupported(
            "CSV requires a top-level sequence of record rows",
        ));
    };
    fields
        .iter()
        .filter(|field| !field.skip)
        .try_for_each(|field| {
            validate_cell_shape(&field.value_shape())
                .map_err(|error| error.at_field(field.wire_name.clone()))
        })?;
    Ok(CsvRowShape {
        fields,
        policy: *policy,
    })
}

fn validate_cell_shape(shape: &TypeShape) -> Result<()> {
    match shape {
        TypeShape::Unit
        | TypeShape::Bool
        | TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::I128
        | TypeShape::Isize
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32
        | TypeShape::U64
        | TypeShape::U128
        | TypeShape::Usize
        | TypeShape::F32
        | TypeShape::F64
        | TypeShape::String
        | TypeShape::Char
        | TypeShape::Bytes { .. } => Ok(()),
        TypeShape::Option(inner) => validate_cell_shape(inner),
        TypeShape::Seq(_)
        | TypeShape::Map { .. }
        | TypeShape::Record { .. }
        | TypeShape::Enum { .. }
        | TypeShape::Named(_) => Err(DataError::unsupported(format!(
            "CSV cell shape {} is not supported",
            shape.type_name()
        ))),
    }
}

fn csv_headers(fields: &[FieldShape]) -> Vec<String> {
    fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| field.wire_name.clone())
        .collect()
}

fn write_row(
    writer: &mut csv::Writer<Vec<u8>>,
    row: &Value,
    row_shape: CsvRowShape<'_>,
    headers: &[String],
    row_index: usize,
) -> Result<()> {
    let record = row.as_record().map_err(|error| error.at_index(row_index))?;
    reject_unknown_fields(record.keys(), row_shape.fields, row_shape.policy)
        .map_err(|error| error.at_index(row_index))?;
    let values = row_shape
        .fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| {
            let shape = field.value_shape();
            match record.get(&field.wire_name) {
                Some(value) => encode_cell(value, &shape),
                None if matches!(shape, TypeShape::Option(_)) => Ok(String::new()),
                None => Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("missing CSV field `{}`", field.wire_name),
                )),
            }
            .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
        })
        .collect::<Result<Vec<_>>>()?;
    debug_assert_eq!(headers.len(), values.len());
    writer.write_record(values).map_err(|error| {
        DataError::new(DataErrorKind::InvalidEncoding, error.to_string()).at_index(row_index)
    })
}

fn validate_headers(headers: &[String], row_shape: CsvRowShape<'_>) -> Result<()> {
    reject_duplicate_headers(headers)?;
    reject_unknown_fields(headers.iter(), row_shape.fields, row_shape.policy)?;
    let present = headers.iter().map(String::as_str).collect::<BTreeSet<_>>();
    row_shape
        .fields
        .iter()
        .filter(|field| !field.skip)
        .try_for_each(|field| {
            if present.contains(field.wire_name.as_str()) {
                Ok(())
            } else {
                Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("missing CSV column `{}`", field.wire_name),
                )
                .at_field(field.wire_name.clone()))
            }
        })
}

fn reject_duplicate_headers(headers: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();
    headers.iter().try_for_each(|header| {
        if seen.insert(header.as_str()) {
            Ok(())
        } else {
            Err(DataError::new(
                DataErrorKind::DuplicateField,
                format!("duplicate CSV column `{header}`"),
            )
            .at_field(header.clone()))
        }
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
                format!("unknown CSV field `{name}`"),
            )
            .at_field(name.clone())
        })
        .next()
        .map_or(Ok(()), Err)
}

fn row_indexes(headers: &[String], fields: &[FieldShape]) -> BTreeMap<String, usize> {
    fields
        .iter()
        .filter(|field| !field.skip)
        .filter_map(|field| {
            headers
                .iter()
                .position(|header| header == &field.wire_name)
                .map(|index| (field.wire_name.clone(), index))
        })
        .collect()
}

fn decode_row(
    record: &csv::StringRecord,
    fields: &[FieldShape],
    row_indexes: &BTreeMap<String, usize>,
    row_index: usize,
    budget: &mut DecodeBudget<'_>,
) -> Result<Value> {
    budget.enter_node()?;
    budget.map_len(fields.iter().filter(|field| !field.skip).count())?;
    let row = fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| {
            let shape = field.value_shape();
            let value = row_indexes
                .get(&field.wire_name)
                .and_then(|index| record.get(*index))
                .ok_or_else(|| {
                    DataError::new(
                        DataErrorKind::MissingField,
                        format!("missing CSV column `{}`", field.wire_name),
                    )
                })?;
            decode_cell(value, &shape, budget)
                .map(|value| (field.wire_name.clone(), value))
                .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    budget.exit_node();
    Ok(Value::Record(row))
}

fn encode_cell(value: &Value, shape: &TypeShape) -> Result<String> {
    match shape {
        TypeShape::Option(inner) => match value {
            Value::Unit => Ok(String::new()),
            other => encode_cell(other, inner),
        },
        TypeShape::Unit => match value {
            Value::Unit => Ok(String::new()),
            other => Err(DataError::invalid_type("unit", other.type_name())),
        },
        TypeShape::Bool => match value {
            Value::Bool(value) => Ok(value.to_string()),
            other => Err(DataError::invalid_type("bool", other.type_name())),
        },
        TypeShape::String => match value {
            Value::String(value) => Ok(value.clone()),
            other => Err(DataError::invalid_type("string", other.type_name())),
        },
        TypeShape::Char => match value {
            Value::Char(value) => Ok(value.to_string()),
            other => Err(DataError::invalid_type("char", other.type_name())),
        },
        TypeShape::Bytes { format } => match value {
            Value::Bytes(bytes) => encode_bytes(bytes.as_slice(), *format),
            other => Err(DataError::invalid_type("bytes", other.type_name())),
        },
        TypeShape::F32 | TypeShape::F64 => encode_float_cell(value, shape),
        TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::I128
        | TypeShape::Isize
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32
        | TypeShape::U64
        | TypeShape::U128
        | TypeShape::Usize => encode_integer_cell(value, shape),
        other => Err(DataError::unsupported(format!(
            "CSV cell shape {} is not supported",
            other.type_name()
        ))),
    }
}

fn decode_cell(value: &str, shape: &TypeShape, budget: &DecodeBudget<'_>) -> Result<Value> {
    match shape {
        TypeShape::Option(inner) if value.is_empty() => Ok(Value::Unit),
        TypeShape::Option(inner) => decode_cell(value, inner, budget),
        TypeShape::Unit if value.is_empty() => Ok(Value::Unit),
        TypeShape::Unit => Err(DataError::invalid_type(
            "empty unit cell",
            "non-empty string",
        )),
        TypeShape::Bool => parse_bool(value),
        TypeShape::String => {
            budget.string_len(value.len())?;
            Ok(Value::String(value.to_owned()))
        }
        TypeShape::Char => parse_char(value),
        TypeShape::Bytes { format } => {
            budget.string_len(value.len())?;
            let bytes = decode_bytes(value, *format, budget)?;
            Ok(Value::Bytes(Bytes::new(bytes)))
        }
        TypeShape::F32 | TypeShape::F64 => parse_float(value, shape),
        TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::I128
        | TypeShape::Isize
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32
        | TypeShape::U64
        | TypeShape::U128
        | TypeShape::Usize => parse_integer(value, shape),
        other => Err(DataError::unsupported(format!(
            "CSV cell shape {} is not supported",
            other.type_name()
        ))),
    }
}

fn encode_integer_cell(value: &Value, shape: &TypeShape) -> Result<String> {
    let Value::Number(number) = value else {
        return Err(DataError::invalid_type("number", value.type_name()));
    };
    match number {
        Number::I(value)
            if shape
                .signed_bounds()
                .is_some_and(|(min, max)| *value >= min && *value <= max) =>
        {
            Ok(value.to_string())
        }
        Number::U(value) if shape.unsigned_max().is_some_and(|max| *value <= max) => {
            Ok(value.to_string())
        }
        Number::I(_) | Number::U(_) => Err(DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("number is out of range for {}", shape.type_name()),
        )),
        Number::F32(_) | Number::F64(_) => {
            Err(DataError::invalid_type("integer", number.type_name()))
        }
    }
}

fn encode_float_cell(value: &Value, shape: &TypeShape) -> Result<String> {
    let Value::Number(number) = value else {
        return Err(DataError::invalid_type("number", value.type_name()));
    };
    match (shape, number) {
        (TypeShape::F32, Number::F32(value)) if value.is_finite() => Ok(value.to_string()),
        (TypeShape::F64, Number::F64(value)) if value.is_finite() => Ok(value.to_string()),
        (TypeShape::F32 | TypeShape::F64, Number::F32(_) | Number::F64(_)) => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "CSV floats must be finite",
        )),
        _ => Err(DataError::invalid_type(
            shape.type_name(),
            number.type_name(),
        )),
    }
}

fn parse_bool(value: &str) -> Result<Value> {
    match value {
        "true" => Ok(Value::Bool(true)),
        "false" => Ok(Value::Bool(false)),
        _ => Err(DataError::invalid_type(
            "bool literal true or false",
            "string",
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

fn parse_integer(value: &str, shape: &TypeShape) -> Result<Value> {
    if shape.signed_bounds().is_some() {
        let parsed = value
            .parse::<i128>()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let (min, max) = shape.signed_bounds().expect("signed shape checked above");
        if parsed < min || parsed > max {
            return Err(DataError::new(
                DataErrorKind::NumberOutOfRange,
                format!("number is out of range for {}", shape.type_name()),
            ));
        }
        return Ok(Value::Number(Number::I(parsed)));
    }
    let parsed = value
        .parse::<u128>()
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    let max = shape
        .unsigned_max()
        .expect("unsigned shape checked by caller");
    if parsed > max {
        return Err(DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("number is out of range for {}", shape.type_name()),
        ));
    }
    Ok(Value::Number(Number::U(parsed)))
}

fn parse_float(value: &str, shape: &TypeShape) -> Result<Value> {
    match shape {
        TypeShape::F32 => {
            let parsed = value.parse::<f32>().map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })?;
            if parsed.is_finite() {
                Ok(Value::Number(Number::F32(parsed)))
            } else {
                Err(DataError::new(
                    DataErrorKind::InvalidEncoding,
                    "CSV floats must be finite",
                ))
            }
        }
        TypeShape::F64 => {
            let parsed = value.parse::<f64>().map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })?;
            if parsed.is_finite() {
                Ok(Value::Number(Number::F64(parsed)))
            } else {
                Err(DataError::new(
                    DataErrorKind::InvalidEncoding,
                    "CSV floats must be finite",
                ))
            }
        }
        other => Err(DataError::invalid_type("float", other.type_name())),
    }
}

fn encode_bytes(bytes: &[u8], format: BytesFormat) -> Result<String> {
    match format {
        BytesFormat::Binary | BytesFormat::Base64 => Ok(BASE64_STANDARD.encode(bytes)),
        BytesFormat::Hex => {
            let mut encoded = String::with_capacity(bytes.len() * 2);
            bytes
                .iter()
                .try_for_each(|byte| write!(&mut encoded, "{byte:02x}"))
                .expect("writing to String cannot fail");
            Ok(encoded)
        }
        BytesFormat::Array => Err(DataError::unsupported(
            "CSV bytes cannot use array representation",
        )),
    }
}

fn decode_bytes(value: &str, format: BytesFormat, budget: &DecodeBudget<'_>) -> Result<Vec<u8>> {
    let bytes = match format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            reject_base64_len_over_budget(value, budget)?;
            BASE64_STANDARD.decode(value.as_bytes()).map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })?
        }
        BytesFormat::Hex => {
            reject_hex_len_over_budget(value, budget)?;
            decode_hex(value)?
        }
        BytesFormat::Array => Err(DataError::unsupported(
            "CSV bytes cannot use array representation",
        ))?,
    };
    budget.bytes_len(bytes.len())?;
    Ok(bytes)
}

fn reject_base64_len_over_budget(value: &str, budget: &DecodeBudget<'_>) -> Result<()> {
    let max_encoded_len = budget
        .max_bytes_len()
        .saturating_add(2)
        .checked_div(3)
        .unwrap_or(usize::MAX)
        .saturating_mul(4);
    if value.len() > max_encoded_len {
        return budget.bytes_len(decoded_len_estimate(value.len()));
    }
    Ok(())
}

fn reject_hex_len_over_budget(value: &str, budget: &DecodeBudget<'_>) -> Result<()> {
    if value.len() > budget.max_bytes_len().saturating_mul(2) {
        return budget.bytes_len(value.len().saturating_add(1) / 2);
    }
    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    let chunks = value.as_bytes().chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "hex byte string has odd length",
        ));
    }
    chunks
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })?;
            u8::from_str_radix(text, 16)
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        })
        .collect()
}
