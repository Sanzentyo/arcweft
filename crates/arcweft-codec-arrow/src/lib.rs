#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::Arc;

use arcweft_data::{
    Bytes, Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions, EncodeOptions, FieldShape,
    FormatId, Number, RecordPolicy, Result, TypeShape, Value,
};
use arrow::array::{
    Array, ArrayRef, BinaryArray, BooleanArray, Float32Array, Float64Array, Int64Array, NullArray,
    StringArray, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use bytes::Bytes as ByteBuffer;
use ipc_preflight::preflight_arrow_ipc_buffers;
use parquet_preflight::preflight_parquet_buffers;

mod ipc_preflight;
mod parquet_preflight;

#[derive(Clone, Copy, Debug, Default)]
pub struct ArrowIpcCodec;

#[derive(Clone, Copy, Debug, Default)]
pub struct ParquetCodec;

impl Codec for ArrowIpcCodec {
    fn id(&self) -> FormatId {
        FormatId::new("arrow-ipc")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/vnd.apache.arrow.file", "application/x-arrow"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["arrow", "feather"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let batch = value_to_batch(value, shape)?;
        let mut output = Vec::new();
        let mut writer = arrow::ipc::writer::FileWriter::try_new(&mut output, &batch.schema())
            .map_err(arrow_error)?;
        writer.write(&batch).map_err(arrow_error)?;
        writer.finish().map_err(arrow_error)?;
        drop(writer);
        Ok(output)
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let row_shape = arrow_row_shape(shape)?;
        preflight_arrow_ipc_buffers(input, row_shape, &options.limits)?;
        let reader = arrow::ipc::reader::FileReader::try_new(Cursor::new(input), None)
            .map_err(arrow_error)?;
        budget.enter_node()?;
        let mut rows = Vec::new();
        let mut rows_seen = 0;
        for batch in reader {
            let batch = batch.map_err(arrow_error)?;
            let batch_rows = batch_to_rows(&batch, row_shape, &mut budget, rows_seen)?;
            rows_seen += batch.num_rows();
            rows.extend(batch_rows);
        }
        budget.exit_node();
        let value = Value::Seq(rows);
        options.limits.validate(&value)?;
        Ok(value)
    }
}

impl Codec for ParquetCodec {
    fn id(&self) -> FormatId {
        FormatId::new("parquet")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/vnd.apache.parquet", "application/x-parquet"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["parquet"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let batch = value_to_batch(value, shape)?;
        let mut output = Vec::new();
        let mut writer = parquet::arrow::ArrowWriter::try_new(&mut output, batch.schema(), None)
            .map_err(arrow_error)?;
        writer.write(&batch).map_err(arrow_error)?;
        writer.close().map_err(arrow_error)?;
        Ok(output)
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let row_shape = arrow_row_shape(shape)?;
        let bytes = ByteBuffer::copy_from_slice(input);
        let builder =
            parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(bytes.clone())
                .map_err(arrow_error)?;
        reject_parquet_row_count(
            builder.metadata().file_metadata().num_rows(),
            &options.limits,
        )?;
        preflight_parquet_buffers(&bytes, builder.metadata(), row_shape, &options.limits)?;
        let reader = builder
            .with_batch_size(parquet_decode_batch_size(options.limits.max_sequence_len))
            .build()
            .map_err(arrow_error)?;
        budget.enter_node()?;
        let mut rows = Vec::new();
        let mut rows_seen = 0;
        for batch in reader {
            let batch = batch.map_err(arrow_error)?;
            let batch_rows = batch_to_rows(&batch, row_shape, &mut budget, rows_seen)?;
            rows_seen += batch.num_rows();
            rows.extend(batch_rows);
        }
        budget.exit_node();
        let value = Value::Seq(rows);
        options.limits.validate(&value)?;
        Ok(value)
    }
}

#[derive(Clone, Copy)]
struct ArrowRowShape<'a> {
    fields: &'a [FieldShape],
    policy: RecordPolicy,
}

fn arrow_row_shape(shape: &TypeShape) -> Result<ArrowRowShape<'_>> {
    let TypeShape::Seq(row_shape) = shape else {
        return Err(DataError::unsupported(
            "Arrow and Parquet require a top-level sequence of record rows",
        ));
    };
    let TypeShape::Record { fields, policy, .. } = row_shape.as_ref() else {
        return Err(DataError::unsupported(
            "Arrow and Parquet require a top-level sequence of record rows",
        ));
    };
    fields
        .iter()
        .filter(|field| !field.skip)
        .try_for_each(|field| {
            arrow_data_type(&field.value_shape())
                .map(|_| ())
                .map_err(|error| error.at_field(field.wire_name.clone()))
        })?;
    Ok(ArrowRowShape {
        fields,
        policy: *policy,
    })
}

fn value_to_batch(value: &Value, shape: &TypeShape) -> Result<RecordBatch> {
    let row_shape = arrow_row_shape(shape)?;
    let rows = value.as_seq()?;
    let fields = row_shape
        .fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| {
            let shape = field.value_shape();
            arrow_data_type(&shape).map(|data_type| {
                Field::new(&field.wire_name, data_type, is_nullable_arrow_shape(&shape))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let arrays = row_shape
        .fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| build_array(rows, field, row_shape.fields, row_shape.policy))
        .collect::<Vec<_>>();
    let schema = Arc::new(Schema::new(fields));
    let arrays = arrays.into_iter().collect::<Result<Vec<_>>>()?;
    RecordBatch::try_new(schema, arrays).map_err(arrow_error)
}

fn arrow_data_type(shape: &TypeShape) -> Result<DataType> {
    match shape {
        TypeShape::Option(inner) => arrow_data_type(inner),
        TypeShape::Unit => Ok(DataType::Null),
        TypeShape::Bool => Ok(DataType::Boolean),
        TypeShape::I8 | TypeShape::I16 | TypeShape::I32 | TypeShape::I64 | TypeShape::Isize => {
            Ok(DataType::Int64)
        }
        TypeShape::U8 | TypeShape::U16 | TypeShape::U32 | TypeShape::U64 | TypeShape::Usize => {
            Ok(DataType::UInt64)
        }
        TypeShape::F32 => Ok(DataType::Float32),
        TypeShape::F64 => Ok(DataType::Float64),
        TypeShape::String | TypeShape::Char => Ok(DataType::Utf8),
        TypeShape::Bytes { .. } => Ok(DataType::Binary),
        TypeShape::I128
        | TypeShape::U128
        | TypeShape::Seq(_)
        | TypeShape::Map { .. }
        | TypeShape::Record { .. }
        | TypeShape::Enum { .. }
        | TypeShape::Named(_) => Err(DataError::unsupported(format!(
            "Arrow scalar shape {} is not supported",
            shape.type_name()
        ))),
    }
}

fn is_nullable_arrow_shape(shape: &TypeShape) -> bool {
    matches!(shape, TypeShape::Option(_) | TypeShape::Unit)
}

fn build_array(
    rows: &[Value],
    field: &FieldShape,
    fields: &[FieldShape],
    policy: RecordPolicy,
) -> Result<ArrayRef> {
    let shape = field.value_shape();
    match arrow_data_type(&shape)? {
        DataType::Null => Ok(Arc::new(NullArray::new(rows.len())) as ArrayRef),
        DataType::Boolean => Ok(Arc::new(BooleanArray::from(
            rows.iter()
                .enumerate()
                .map(|(index, row)| bool_cell(row, field, fields, &shape, policy, index))
                .collect::<Result<Vec<_>>>()?,
        )) as ArrayRef),
        DataType::Int64 => Ok(Arc::new(Int64Array::from(
            rows.iter()
                .enumerate()
                .map(|(index, row)| i64_cell(row, field, fields, &shape, policy, index))
                .collect::<Result<Vec<_>>>()?,
        )) as ArrayRef),
        DataType::UInt64 => Ok(Arc::new(UInt64Array::from(
            rows.iter()
                .enumerate()
                .map(|(index, row)| u64_cell(row, field, fields, &shape, policy, index))
                .collect::<Result<Vec<_>>>()?,
        )) as ArrayRef),
        DataType::Float32 => Ok(Arc::new(Float32Array::from(
            rows.iter()
                .enumerate()
                .map(|(index, row)| f32_cell(row, field, fields, &shape, policy, index))
                .collect::<Result<Vec<_>>>()?,
        )) as ArrayRef),
        DataType::Float64 => Ok(Arc::new(Float64Array::from(
            rows.iter()
                .enumerate()
                .map(|(index, row)| f64_cell(row, field, fields, &shape, policy, index))
                .collect::<Result<Vec<_>>>()?,
        )) as ArrayRef),
        DataType::Binary => {
            let cells = rows
                .iter()
                .enumerate()
                .map(|(index, row)| bytes_cell(row, field, fields, &shape, policy, index))
                .collect::<Result<Vec<_>>>()?;
            let borrowed = cells
                .iter()
                .map(std::option::Option::as_deref)
                .collect::<Vec<_>>();
            Ok(Arc::new(BinaryArray::from(borrowed)))
        }
        DataType::Utf8 => Ok(Arc::new(StringArray::from(
            rows.iter()
                .enumerate()
                .map(|(index, row)| string_cell(row, field, fields, &shape, policy, index))
                .collect::<Result<Vec<_>>>()?,
        )) as ArrayRef),
        other => Err(DataError::unsupported(format!(
            "Arrow type {other:?} is not mapped yet"
        ))),
    }
}

fn reject_unknown_fields(
    record: &BTreeMap<String, Value>,
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
    record
        .keys()
        .find(|name| !known.contains(name.as_str()))
        .map(|name| {
            DataError::new(
                DataErrorKind::UnknownField,
                format!("unknown Arrow field `{name}`"),
            )
            .at_field(name.clone())
        })
        .map_or(Ok(()), Err)
}

fn row_value<'a>(
    row: &'a Value,
    field: &FieldShape,
    shape: &TypeShape,
    fields: &[FieldShape],
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<&'a Value>> {
    let record = row.as_record().map_err(|error| error.at_index(row_index))?;
    reject_unknown_fields(record, fields, policy).map_err(|error| error.at_index(row_index))?;
    match record.get(&field.wire_name) {
        Some(Value::Unit) if matches!(shape, TypeShape::Option(_) | TypeShape::Unit) => Ok(None),
        Some(value) => Ok(Some(value)),
        None if matches!(shape, TypeShape::Option(_) | TypeShape::Unit) => Ok(None),
        None => Err(DataError::new(
            DataErrorKind::MissingField,
            format!("missing Arrow field `{}`", field.wire_name),
        )
        .at_field(field.wire_name.clone())
        .at_index(row_index)),
    }
}

fn cell_value<'a>(
    row: &'a Value,
    field: &FieldShape,
    fields: &[FieldShape],
    shape: &TypeShape,
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<&'a Value>> {
    row_value(row, field, shape, fields, policy, row_index)
}

fn bool_cell(
    row: &Value,
    field: &FieldShape,
    fields: &[FieldShape],
    shape: &TypeShape,
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<bool>> {
    let Some(value) = cell_value(row, field, fields, shape, policy, row_index)? else {
        return Ok(None);
    };
    match option_inner(shape) {
        TypeShape::Bool => match value {
            Value::Bool(value) => Ok(Some(*value)),
            other => Err(DataError::invalid_type("bool", other.type_name())),
        },
        _ => unreachable!("bool column uses bool shape"),
    }
    .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
}

fn i64_cell(
    row: &Value,
    field: &FieldShape,
    fields: &[FieldShape],
    shape: &TypeShape,
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<i64>> {
    let Some(value) = cell_value(row, field, fields, shape, policy, row_index)? else {
        return Ok(None);
    };
    let Value::Number(number) = value else {
        return Err(DataError::invalid_type("number", value.type_name())
            .at_field(field.wire_name.clone())
            .at_index(row_index));
    };
    match number {
        Number::I(value)
            if option_inner(shape)
                .signed_bounds()
                .is_some_and(|(min, max)| *value >= min && *value <= max) =>
        {
            i64::try_from(*value)
                .map(Some)
                .map_err(|_| range_error(shape))
        }
        Number::U(value)
            if option_inner(shape)
                .signed_bounds()
                .is_some_and(|(_, max)| i128::try_from(*value).is_ok_and(|value| value <= max)) =>
        {
            i64::try_from(*value)
                .map(Some)
                .map_err(|_| range_error(shape))
        }
        Number::I(_) | Number::U(_) => Err(range_error(shape)),
        Number::F32(_) | Number::F64(_) => {
            Err(DataError::invalid_type("integer", number.type_name()))
        }
    }
    .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
}

fn u64_cell(
    row: &Value,
    field: &FieldShape,
    fields: &[FieldShape],
    shape: &TypeShape,
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<u64>> {
    let Some(value) = cell_value(row, field, fields, shape, policy, row_index)? else {
        return Ok(None);
    };
    let Value::Number(number) = value else {
        return Err(DataError::invalid_type("number", value.type_name())
            .at_field(field.wire_name.clone())
            .at_index(row_index));
    };
    match number {
        Number::U(value)
            if option_inner(shape)
                .unsigned_max()
                .is_some_and(|max| *value <= max) =>
        {
            u64::try_from(*value)
                .map(Some)
                .map_err(|_| range_error(shape))
        }
        Number::I(value)
            if *value >= 0
                && option_inner(shape)
                    .unsigned_max()
                    .is_some_and(|max| u128::try_from(*value).is_ok_and(|value| value <= max)) =>
        {
            u64::try_from(*value)
                .map(Some)
                .map_err(|_| range_error(shape))
        }
        Number::I(_) | Number::U(_) => Err(range_error(shape)),
        Number::F32(_) | Number::F64(_) => {
            Err(DataError::invalid_type("integer", number.type_name()))
        }
    }
    .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
}

fn f32_cell(
    row: &Value,
    field: &FieldShape,
    fields: &[FieldShape],
    shape: &TypeShape,
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<f32>> {
    let Some(value) = cell_value(row, field, fields, shape, policy, row_index)? else {
        return Ok(None);
    };
    match value {
        Value::Number(Number::F32(value)) if value.is_finite() => Ok(Some(*value)),
        Value::Number(Number::F32(_)) => Err(non_finite_error()),
        Value::Number(number) => Err(DataError::invalid_type("f32", number.type_name())),
        other => Err(DataError::invalid_type("f32", other.type_name())),
    }
    .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
}

fn f64_cell(
    row: &Value,
    field: &FieldShape,
    fields: &[FieldShape],
    shape: &TypeShape,
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<f64>> {
    let Some(value) = cell_value(row, field, fields, shape, policy, row_index)? else {
        return Ok(None);
    };
    match value {
        Value::Number(Number::F64(value)) if value.is_finite() => Ok(Some(*value)),
        Value::Number(Number::F64(_)) => Err(non_finite_error()),
        Value::Number(number) => Err(DataError::invalid_type("f64", number.type_name())),
        other => Err(DataError::invalid_type("f64", other.type_name())),
    }
    .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
}

fn string_cell(
    row: &Value,
    field: &FieldShape,
    fields: &[FieldShape],
    shape: &TypeShape,
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<String>> {
    let Some(value) = cell_value(row, field, fields, shape, policy, row_index)? else {
        return Ok(None);
    };
    match option_inner(shape) {
        TypeShape::String => match value {
            Value::String(value) => Ok(Some(value.clone())),
            other => Err(DataError::invalid_type("string", other.type_name())),
        },
        TypeShape::Char => match value {
            Value::Char(value) => Ok(Some(value.to_string())),
            other => Err(DataError::invalid_type("char", other.type_name())),
        },
        _ => unreachable!("string column uses string-compatible shape"),
    }
    .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
}

fn bytes_cell(
    row: &Value,
    field: &FieldShape,
    fields: &[FieldShape],
    shape: &TypeShape,
    policy: RecordPolicy,
    row_index: usize,
) -> Result<Option<Vec<u8>>> {
    let Some(value) = cell_value(row, field, fields, shape, policy, row_index)? else {
        return Ok(None);
    };
    match value {
        Value::Bytes(bytes) => Ok(Some(bytes.as_slice().to_vec())),
        other => Err(DataError::invalid_type("bytes", other.type_name())),
    }
    .map_err(|error| error.at_field(field.wire_name.clone()).at_index(row_index))
}

fn batch_to_rows(
    batch: &RecordBatch,
    row_shape: ArrowRowShape<'_>,
    budget: &mut DecodeBudget<'_>,
    row_offset: usize,
) -> Result<Vec<Value>> {
    reject_unknown_columns(batch, row_shape)?;
    (0..batch.num_rows())
        .map(|row| {
            budget.sequence_item(row_offset + row + 1)?;
            with_budget_node(budget, |budget| {
                let field_count = row_shape.fields.iter().filter(|field| !field.skip).count();
                budget.map_len(field_count)?;
                row_shape
                    .fields
                    .iter()
                    .filter(|field| !field.skip)
                    .map(|field| {
                        let col = batch
                            .schema()
                            .index_of(&field.wire_name)
                            .map_err(arrow_error)?;
                        column_value(batch.column(col).as_ref(), field, row, budget)
                            .map(|value| (field.wire_name.clone(), value))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()
                    .map(Value::Record)
            })
        })
        .collect()
}

fn reject_unknown_columns(batch: &RecordBatch, row_shape: ArrowRowShape<'_>) -> Result<()> {
    if !row_shape.policy.deny_unknown_fields {
        return Ok(());
    }
    let known = row_shape
        .fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| field.wire_name.as_str())
        .collect::<BTreeSet<_>>();
    batch
        .schema()
        .fields()
        .iter()
        .find(|field| !known.contains(field.name().as_str()))
        .map(|field| {
            DataError::new(
                DataErrorKind::UnknownField,
                format!("unknown Arrow column `{}`", field.name()),
            )
            .at_field(field.name().clone())
        })
        .map_or(Ok(()), Err)
}

fn column_value(
    array: &dyn Array,
    field: &FieldShape,
    row: usize,
    budget: &mut DecodeBudget<'_>,
) -> Result<Value> {
    let shape = field.value_shape();
    with_budget_node(budget, |budget| {
        if array.is_null(row) {
            return if is_nullable_arrow_shape(&shape) {
                Ok(Value::Unit)
            } else {
                Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("null in required Arrow column `{}`", field.wire_name),
                )
                .at_field(field.wire_name.clone())
                .at_index(row))
            };
        }
        let data_type = arrow_data_type(&shape)?;
        match data_type {
            DataType::Null => Ok(Value::Unit),
            DataType::Boolean => decode_bool_column(array, row),
            DataType::Int64 => decode_i64_column(array, row, &shape),
            DataType::UInt64 => decode_u64_column(array, row, &shape),
            DataType::Float32 => decode_f32_column(array, row),
            DataType::Float64 => decode_f64_column(array, row),
            DataType::Binary => decode_binary_column(array, row, budget),
            DataType::Utf8 => decode_utf8_column(array, row, &shape, budget),
            other => Err(DataError::unsupported(format!(
                "Arrow type {other:?} is not mapped yet"
            ))),
        }
    })
}

fn with_budget_node<T>(
    budget: &mut DecodeBudget<'_>,
    f: impl FnOnce(&mut DecodeBudget<'_>) -> Result<T>,
) -> Result<T> {
    budget.enter_node()?;
    let result = f(budget);
    budget.exit_node();
    result
}

fn reject_parquet_row_count(row_count: i64, limits: &arcweft_data::DecodeLimits) -> Result<()> {
    let row_count = usize::try_from(row_count).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("Parquet row count {row_count} is negative or too large"),
        )
    })?;
    if row_count > limits.max_sequence_len {
        Err(DataError::limit(format!(
            "sequence length {row_count} exceeds {}",
            limits.max_sequence_len
        )))
    } else if row_count > limits.max_collection_items {
        Err(DataError::limit(format!(
            "collection item budget exhausted by length {row_count}"
        )))
    } else {
        Ok(())
    }
}

fn parquet_decode_batch_size(max_sequence_len: usize) -> usize {
    max_sequence_len.clamp(1, 8192)
}

fn decode_bool_column(array: &dyn Array, row: usize) -> Result<Value> {
    Ok(Value::Bool(
        array
            .as_any()
            .downcast_ref::<BooleanArray>()
            .ok_or_else(|| type_error(&DataType::Boolean))?
            .value(row),
    ))
}

fn decode_i64_column(array: &dyn Array, row: usize, shape: &TypeShape) -> Result<Value> {
    let value = i128::from(
        array
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| type_error(&DataType::Int64))?
            .value(row),
    );
    let shape = option_inner(shape);
    if shape
        .signed_bounds()
        .is_some_and(|(min, max)| value >= min && value <= max)
    {
        Ok(Value::Number(Number::I(value)))
    } else {
        Err(range_error(shape))
    }
}

fn decode_u64_column(array: &dyn Array, row: usize, shape: &TypeShape) -> Result<Value> {
    let value = u128::from(
        array
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or_else(|| type_error(&DataType::UInt64))?
            .value(row),
    );
    let shape = option_inner(shape);
    if shape.unsigned_max().is_some_and(|max| value <= max) {
        Ok(Value::Number(Number::U(value)))
    } else {
        Err(range_error(shape))
    }
}

fn decode_f32_column(array: &dyn Array, row: usize) -> Result<Value> {
    let value = array
        .as_any()
        .downcast_ref::<Float32Array>()
        .ok_or_else(|| type_error(&DataType::Float32))?
        .value(row);
    if value.is_finite() {
        Ok(Value::Number(Number::F32(value)))
    } else {
        Err(non_finite_error())
    }
}

fn decode_f64_column(array: &dyn Array, row: usize) -> Result<Value> {
    let value = array
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| type_error(&DataType::Float64))?
        .value(row);
    if value.is_finite() {
        Ok(Value::Number(Number::F64(value)))
    } else {
        Err(non_finite_error())
    }
}

fn decode_binary_column(array: &dyn Array, row: usize, budget: &DecodeBudget<'_>) -> Result<Value> {
    let value = array
        .as_any()
        .downcast_ref::<BinaryArray>()
        .ok_or_else(|| type_error(&DataType::Binary))?
        .value(row);
    budget.bytes_len(value.len())?;
    Ok(Value::Bytes(Bytes::new(value.to_vec())))
}

fn decode_utf8_column(
    array: &dyn Array,
    row: usize,
    shape: &TypeShape,
    budget: &DecodeBudget<'_>,
) -> Result<Value> {
    let value = array
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| type_error(&DataType::Utf8))?
        .value(row);
    budget.string_len(value.len())?;
    match option_inner(shape) {
        TypeShape::String => Ok(Value::String(value.to_owned())),
        TypeShape::Char => decode_char(value),
        _ => unreachable!("utf8 data type uses string-compatible shape"),
    }
}

fn type_error(data_type: &DataType) -> DataError {
    DataError::new(
        DataErrorKind::InvalidType,
        format!("array type mismatch for {data_type:?}"),
    )
}

fn arrow_error(error: impl std::fmt::Display) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
}

fn option_inner(shape: &TypeShape) -> &TypeShape {
    match shape {
        TypeShape::Option(inner) => inner,
        other => other,
    }
}

fn decode_char(value: &str) -> Result<Value> {
    let mut chars = value.chars();
    let Some(ch) = chars.next() else {
        return Err(DataError::invalid_type("single char", "empty string"));
    };
    if chars.next().is_some() {
        return Err(DataError::invalid_type("single char", "multi-char string"));
    }
    Ok(Value::Char(ch))
}

fn range_error(shape: &TypeShape) -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        format!(
            "number is out of range for {}",
            option_inner(shape).type_name()
        ),
    )
}

fn non_finite_error() -> DataError {
    DataError::new(
        DataErrorKind::InvalidEncoding,
        "non-finite floats are not valid Arcweft data values",
    )
}
