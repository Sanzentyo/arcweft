#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use arcweft_data::{
    Bytes, BytesFormat, Codec, DataError, DataErrorKind, DecodeBudget, DecodeLimits, DecodeOptions,
    EncodeOptions, FieldShape, FormatId, Number, RecordPolicy, Result, ShapeAccess, ShapeRef,
    TypeShape, Value,
};
use base64::{
    decoded_len_estimate,
    prelude::{BASE64_STANDARD, Engine as _},
};
use csv_core::ReadFieldResult;

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
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let row_shape = csv_row_shape(shape, access)?;
        let rows = value.as_seq()?;
        let headers = csv_headers(row_shape.fields());
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer
            .write_record(&headers)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        rows.iter().enumerate().try_for_each(|(index, row)| {
            write_row(&mut writer, row, &row_shape, &headers, index, access)
        })?;
        writer
            .into_inner()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let row_shape = csv_row_shape(shape, access)?;
        preflight_csv_budget(input, &row_shape, access, &options.limits)?;
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let mut reader = csv::Reader::from_reader(input);
        let headers = reader
            .headers()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?
            .iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        validate_headers(&headers, &row_shape, access)?;
        let row_indexes = row_indexes(&headers, row_shape.fields());
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
                    &row_shape,
                    &row_indexes,
                    row_index,
                    access,
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

fn preflight_csv_budget(
    input: &[u8],
    row_shape: &CsvRowShape,
    access: &dyn ShapeAccess,
    limits: &DecodeLimits,
) -> Result<()> {
    let mut budget = DecodeBudget::new(input.len(), limits)?;
    let mut reader = csv_core::Reader::new();
    let mut remaining = input;
    let mut output = [0_u8; 8 * 1024];
    let mut field_len = 0_usize;
    let mut field_count = 0_usize;
    let mut record_index = 0_usize;
    let mut header_fields = Vec::new();
    let mut header_field = Vec::new();
    let mut byte_formats = Vec::new();
    budget.enter_node()?;
    let result = loop {
        let (result, consumed, produced) = reader.read_field(remaining, &mut output);
        remaining = &remaining[consumed..];
        if record_index == 0 {
            header_field.extend_from_slice(&output[..produced]);
        }
        field_len = match field_len.checked_add(produced) {
            Some(field_len) => field_len,
            None => {
                break Err(DataError::limit(
                    "CSV field length overflow while preflighting budget",
                ));
            }
        };
        if let Err(error) = budget.string_len(field_len) {
            break Err(error);
        }
        match result {
            ReadFieldResult::InputEmpty | ReadFieldResult::OutputFull => {}
            ReadFieldResult::Field { record_end } => {
                let field_index = field_count;
                if record_index == 0 {
                    let header = match std::str::from_utf8(&header_field) {
                        Ok(header) => header.to_owned(),
                        Err(error) => {
                            break Err(DataError::new(
                                DataErrorKind::InvalidEncoding,
                                error.to_string(),
                            ));
                        }
                    };
                    header_fields.push(header);
                    header_field.clear();
                } else if let Some(Some(format)) = byte_formats.get(field_index)
                    && let Err(error) =
                        reject_encoded_csv_bytes_len_over_budget(field_len, *format, &budget)
                {
                    break Err(error);
                }
                field_count = match field_count.checked_add(1) {
                    Some(field_count) => field_count,
                    None => {
                        break Err(DataError::limit(
                            "CSV field count overflow while preflighting budget",
                        ));
                    }
                };
                field_len = 0;
                if record_end {
                    if record_index == 0 {
                        if let Err(error) = validate_headers(&header_fields, row_shape, access) {
                            break Err(error);
                        }
                        byte_formats =
                            match header_byte_formats(&header_fields, row_shape.fields(), access) {
                                Ok(byte_formats) => byte_formats,
                                Err(error) => break Err(error),
                            };
                    } else {
                        if let Err(error) = budget.sequence_item(record_index) {
                            break Err(error);
                        }
                        if let Err(error) = budget.map_len(field_count) {
                            break Err(error);
                        }
                    }
                    record_index = match record_index.checked_add(1) {
                        Some(record_index) => record_index,
                        None => {
                            break Err(DataError::limit(
                                "CSV record count overflow while preflighting budget",
                            ));
                        }
                    };
                    field_count = 0;
                }
            }
            ReadFieldResult::End => break Ok(()),
        }
    };
    budget.exit_node();
    result
}

fn header_byte_formats(
    headers: &[String],
    fields: &[FieldShape],
    access: &dyn ShapeAccess,
) -> Result<Vec<Option<BytesFormat>>> {
    headers
        .iter()
        .map(|header| {
            fields
                .iter()
                .find(|field| !field.skip && field.wire_name == *header)
                .map(|field| {
                    let shape = field
                        .resolve_value_shape(access)
                        .map_err(|error| error.at_field(field.wire_name.clone()))?;
                    bytes_format_for_shape(ShapeRef::Inline(shape.as_ref()), access)
                        .map_err(|error| error.at_field(field.wire_name.clone()))
                })
                .transpose()
                .map(Option::flatten)
        })
        .collect()
}

fn bytes_format_for_shape(
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<Option<BytesFormat>> {
    let shape = shape.resolve(access)?;
    match shape.as_ref() {
        TypeShape::Bytes { format } => Ok(Some(*format)),
        TypeShape::Option(inner) => bytes_format_for_shape(ShapeRef::Inline(inner), access),
        TypeShape::Ref(id) => bytes_format_for_shape(ShapeRef::Id(*id), access),
        _ => Ok(None),
    }
}

fn reject_encoded_csv_bytes_len_over_budget(
    encoded_len: usize,
    format: BytesFormat,
    budget: &DecodeBudget<'_>,
) -> Result<()> {
    match format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            let max_encoded_len = budget
                .max_bytes_len()
                .saturating_add(2)
                .checked_div(3)
                .unwrap_or(usize::MAX)
                .saturating_mul(4);
            if encoded_len > max_encoded_len {
                budget.bytes_len(decoded_len_estimate(encoded_len))
            } else {
                Ok(())
            }
        }
        BytesFormat::Hex => {
            if encoded_len > budget.max_bytes_len().saturating_mul(2) {
                budget.bytes_len(encoded_len.saturating_add(1) / 2)
            } else {
                Ok(())
            }
        }
        BytesFormat::Array => Err(DataError::unsupported(
            "CSV bytes cannot use array representation",
        )),
    }
}

struct CsvRowShape {
    record: TypeShape,
    coordinate: Option<arcweft_data::ShapeId>,
}

impl CsvRowShape {
    fn fields(&self) -> &[FieldShape] {
        let TypeShape::Record { fields, .. } = &self.record else {
            unreachable!()
        };
        fields
    }
    fn policy(&self) -> RecordPolicy {
        let TypeShape::Record { policy, .. } = &self.record else {
            unreachable!()
        };
        *policy
    }
    fn reference(&self) -> ShapeRef<'_> {
        self.coordinate
            .map_or(ShapeRef::Inline(&self.record), ShapeRef::Id)
    }
}

const CSV_OPTION_NONE: &str = "~arcweft-option:none";
const CSV_OPTION_SOME_PREFIX: &str = "~arcweft-option:some:";

fn csv_row_shape(shape: ShapeRef<'_>, access: &dyn ShapeAccess) -> Result<CsvRowShape> {
    let shape = shape.resolve(access)?;
    let TypeShape::Seq(row_shape) = shape.as_ref() else {
        return Err(DataError::unsupported(
            "CSV requires a top-level sequence of record rows",
        ));
    };
    let coordinate = ShapeRef::Inline(row_shape).referenced_id();
    let row_shape = ShapeRef::Inline(row_shape).resolve(access)?;
    let TypeShape::Record { fields, .. } = row_shape.as_ref() else {
        return Err(DataError::unsupported(
            "CSV requires a top-level sequence of record rows",
        ));
    };
    fields
        .iter()
        .filter(|field| !field.skip)
        .try_for_each(|field| {
            let shape = field
                .resolve_value_shape(access)
                .map_err(|error| error.at_field(field.wire_name.clone()))?;
            validate_cell_shape(ShapeRef::Inline(shape.as_ref()), access)
                .map_err(|error| error.at_field(field.wire_name.clone()))
        })?;
    Ok(CsvRowShape {
        record: row_shape.into_owned(),
        coordinate,
    })
}

fn validate_cell_shape(shape: ShapeRef<'_>, access: &dyn ShapeAccess) -> Result<()> {
    let shape = shape.resolve(access)?;
    match shape.as_ref() {
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
        TypeShape::Option(inner) => validate_cell_shape(ShapeRef::Inline(inner), access),
        TypeShape::Ref(id) => validate_cell_shape(ShapeRef::Id(*id), access),
        TypeShape::Seq(_)
        | TypeShape::Map { .. }
        | TypeShape::Tuple(_)
        | TypeShape::Record { .. }
        | TypeShape::Enum { .. } => Err(DataError::unsupported(format!(
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
    row_shape: &CsvRowShape,
    headers: &[String],
    row_index: usize,
    access: &dyn ShapeAccess,
) -> Result<()> {
    let record = row.as_record().map_err(|error| error.at_index(row_index))?;
    reject_unknown_fields(record.keys(), row_shape.fields(), row_shape.policy())
        .map_err(|error| error.at_index(row_index))?;
    let values = row_shape
        .fields()
        .iter()
        .filter(|field| !field.skip)
        .map(|field| {
            let shape = field
                .resolve_value_shape(access)
                .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))?;
            let is_option = matches!(shape.as_ref(), TypeShape::Option(_));
            match record.get(&field.wire_name) {
                Some(value) => encode_cell(value, ShapeRef::Inline(shape.as_ref()), access),
                None if is_option => Ok(CSV_OPTION_NONE.to_owned()),
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

fn validate_headers(
    headers: &[String],
    row_shape: &CsvRowShape,
    access: &dyn ShapeAccess,
) -> Result<()> {
    reject_duplicate_headers(headers)?;
    reject_unknown_fields(headers.iter(), row_shape.fields(), row_shape.policy())?;
    let present = headers.iter().map(String::as_str).collect::<BTreeSet<_>>();
    row_shape
        .fields()
        .iter()
        .filter(|field| !field.skip)
        .try_for_each(|field| {
            if present.contains(field.wire_name.as_str())
                || field.has_default
                || matches!(
                    field.resolve_value_shape(access)?.as_ref(),
                    TypeShape::Option(_)
                )
            {
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
    row_shape: &CsvRowShape,
    row_indexes: &BTreeMap<String, usize>,
    row_index: usize,
    access: &dyn ShapeAccess,
    budget: &mut DecodeBudget<'_>,
) -> Result<Value> {
    budget.enter_node()?;
    budget.map_len(row_shape.fields().len())?;
    let row = row_shape
        .fields()
        .iter()
        .enumerate()
        .map(|(ordinal, field)| {
            let shape = field
                .resolve_value_shape(access)
                .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))?;
            let value = (!field.skip)
                .then(|| {
                    row_indexes
                        .get(&field.wire_name)
                        .and_then(|index| record.get(*index))
                })
                .flatten();
            let Some(value) = value else {
                return field
                    .missing_value(
                        arcweft_data::FieldDefaultRequest::new(row_shape.reference(), ordinal),
                        access,
                    )
                    .map(|value| (field.wire_name.clone(), value))
                    .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index));
            };
            decode_cell(value, ShapeRef::Inline(shape.as_ref()), access, budget)
                .map(|value| (field.wire_name.clone(), value))
                .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    budget.exit_node();
    Ok(Value::Record(row))
}

fn encode_cell(value: &Value, shape_ref: ShapeRef<'_>, access: &dyn ShapeAccess) -> Result<String> {
    let shape = shape_ref.resolve(access)?;
    match shape.as_ref() {
        TypeShape::Ref(id) => encode_cell(value, ShapeRef::Id(*id), access),
        TypeShape::Option(inner) => match value {
            Value::Option(None) => Ok(CSV_OPTION_NONE.to_owned()),
            Value::Option(Some(value)) => {
                let encoded = encode_cell(value, ShapeRef::Inline(inner), access)?;
                Ok(format!(
                    "{CSV_OPTION_SOME_PREFIX}{}",
                    BASE64_STANDARD.encode(encoded.as_bytes())
                ))
            }
            other => Err(DataError::invalid_type("option", other.type_name())),
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
        TypeShape::F32 | TypeShape::F64 => encode_float_cell(value, shape.as_ref()),
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
        | TypeShape::Usize => encode_integer_cell(value, shape.as_ref()),
        other => Err(DataError::unsupported(format!(
            "CSV cell shape {} is not supported",
            other.type_name()
        ))),
    }
}

fn decode_cell(
    value: &str,
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
    budget: &DecodeBudget<'_>,
) -> Result<Value> {
    let shape = shape_ref.resolve(access)?;
    match shape.as_ref() {
        TypeShape::Ref(id) => decode_cell(value, ShapeRef::Id(*id), access, budget),
        TypeShape::Option(inner) if value == CSV_OPTION_NONE => Ok(Value::Option(None)),
        TypeShape::Option(inner) if value.starts_with(CSV_OPTION_SOME_PREFIX) => {
            let encoded = &value[CSV_OPTION_SOME_PREFIX.len()..];
            let bytes = BASE64_STANDARD
                .decode(encoded.as_bytes())
                .map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                })?;
            let payload = String::from_utf8(bytes).map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })?;
            decode_cell(&payload, ShapeRef::Inline(inner), access, budget)
                .map(Box::new)
                .map(Some)
                .map(Value::Option)
        }
        TypeShape::Option(_) => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "CSV option cell is missing its presence marker",
        )),
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
        TypeShape::F32 | TypeShape::F64 => parse_float(value, shape.as_ref()),
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
        | TypeShape::Usize => parse_integer(value, shape.as_ref()),
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
