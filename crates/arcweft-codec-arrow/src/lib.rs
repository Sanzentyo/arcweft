#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::Arc;

use arcweft_data::{
    Bytes, Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Number, Result,
    TypeShape, Value,
};
use arrow::array::{
    Array, ArrayRef, BinaryArray, BooleanArray, Float64Array, Int64Array, StringArray, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use bytes::Bytes as ByteBuffer;

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
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let batch = value_to_batch(value)?;
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
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let reader = arrow::ipc::reader::FileReader::try_new(Cursor::new(input), None)
            .map_err(arrow_error)?;
        let rows = reader
            .map(|batch| {
                batch
                    .map_err(arrow_error)
                    .and_then(|batch| batch_to_rows(&batch))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
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
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let batch = value_to_batch(value)?;
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
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let bytes = ByteBuffer::copy_from_slice(input);
        let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(bytes)
            .map_err(arrow_error)?;
        let reader = builder.build().map_err(arrow_error)?;
        let rows = reader
            .map(|batch| {
                batch
                    .map_err(arrow_error)
                    .and_then(|batch| batch_to_rows(&batch))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let value = Value::Seq(rows);
        options.limits.validate(&value)?;
        Ok(value)
    }
}

fn value_to_batch(value: &Value) -> Result<RecordBatch> {
    let rows = value.as_seq()?;
    let headers = collect_headers(rows)?;
    let fields = headers
        .iter()
        .map(|header| Field::new(header, infer_column_type(rows, header), true))
        .collect::<Vec<_>>();
    let schema = Arc::new(Schema::new(fields));
    let arrays = headers
        .iter()
        .map(|header| build_array(rows, header))
        .collect::<Vec<_>>();
    RecordBatch::try_new(schema, arrays).map_err(arrow_error)
}

fn collect_headers(rows: &[Value]) -> Result<Vec<String>> {
    rows.iter()
        .map(Value::as_record)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flat_map(|record| record.keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .pipe(Ok)
}

fn infer_column_type(rows: &[Value], header: &str) -> DataType {
    rows.iter()
        .filter_map(|row| row.as_record().ok()?.get(header))
        .map(|value| match value {
            Value::Bool(_) => DataType::Boolean,
            Value::Number(Number::I(_)) => DataType::Int64,
            Value::Number(Number::U(_)) => DataType::UInt64,
            Value::Number(Number::F32(_) | Number::F64(_)) => DataType::Float64,
            Value::Bytes(_) => DataType::Binary,
            Value::String(_)
            | Value::Char(_)
            | Value::Unit
            | Value::Seq(_)
            | Value::Map(_)
            | Value::Record(_)
            | Value::Enum { .. } => DataType::Utf8,
        })
        .next()
        .unwrap_or(DataType::Utf8)
}

fn build_array(rows: &[Value], header: &str) -> ArrayRef {
    match infer_column_type(rows, header) {
        DataType::Boolean => Arc::new(BooleanArray::from(
            rows.iter()
                .map(|row| bool_cell(row, header))
                .collect::<Vec<_>>(),
        )),
        DataType::Int64 => Arc::new(Int64Array::from(
            rows.iter()
                .map(|row| i64_cell(row, header))
                .collect::<Vec<_>>(),
        )),
        DataType::UInt64 => Arc::new(UInt64Array::from(
            rows.iter()
                .map(|row| u64_cell(row, header))
                .collect::<Vec<_>>(),
        )),
        DataType::Float64 => Arc::new(Float64Array::from(
            rows.iter()
                .map(|row| f64_cell(row, header))
                .collect::<Vec<_>>(),
        )),
        DataType::Binary => {
            let cells = rows
                .iter()
                .map(|row| bytes_cell(row, header))
                .collect::<Vec<_>>();
            let borrowed = cells
                .iter()
                .map(std::option::Option::as_deref)
                .collect::<Vec<_>>();
            Arc::new(BinaryArray::from(borrowed))
        }
        _ => Arc::new(StringArray::from(
            rows.iter()
                .map(|row| string_cell(row, header))
                .collect::<Vec<_>>(),
        )),
    }
}

fn row_value<'a>(row: &'a Value, header: &str) -> Option<&'a Value> {
    row.as_record().ok()?.get(header)
}

fn bool_cell(row: &Value, header: &str) -> Option<bool> {
    match row_value(row, header)? {
        Value::Bool(value) => Some(*value),
        _ => None,
    }
}

fn i64_cell(row: &Value, header: &str) -> Option<i64> {
    match row_value(row, header)? {
        Value::Number(Number::I(value)) => i64::try_from(*value).ok(),
        Value::Number(Number::U(value)) => i64::try_from(*value).ok(),
        _ => None,
    }
}

fn u64_cell(row: &Value, header: &str) -> Option<u64> {
    match row_value(row, header)? {
        Value::Number(Number::U(value)) => u64::try_from(*value).ok(),
        Value::Number(Number::I(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn f64_cell(row: &Value, header: &str) -> Option<f64> {
    match row_value(row, header)? {
        Value::Number(Number::F32(value)) => Some(f64::from(*value)),
        Value::Number(Number::F64(value)) => Some(*value),
        Value::Number(Number::I(value)) => value.to_string().parse::<f64>().ok(),
        Value::Number(Number::U(value)) => value.to_string().parse::<f64>().ok(),
        _ => None,
    }
}

fn string_cell(row: &Value, header: &str) -> Option<String> {
    row_value(row, header).and_then(Value::stringify_scalar)
}

fn bytes_cell(row: &Value, header: &str) -> Option<Vec<u8>> {
    match row_value(row, header)? {
        Value::Bytes(bytes) => Some(bytes.as_slice().to_vec()),
        _ => None,
    }
}

fn batch_to_rows(batch: &RecordBatch) -> Result<Vec<Value>> {
    (0..batch.num_rows())
        .map(|row| {
            batch
                .schema()
                .fields()
                .iter()
                .enumerate()
                .map(|(col, field)| {
                    column_value(batch.column(col).as_ref(), field.data_type(), row)
                        .map(|value| (field.name().clone(), value))
                })
                .collect::<Result<BTreeMap<_, _>>>()
                .map(Value::Record)
        })
        .collect()
}

fn column_value(array: &dyn Array, data_type: &DataType, row: usize) -> Result<Value> {
    if array.is_null(row) {
        return Ok(Value::Unit);
    }
    match data_type {
        DataType::Boolean => Ok(Value::Bool(
            array
                .as_any()
                .downcast_ref::<BooleanArray>()
                .ok_or_else(|| type_error(data_type))?
                .value(row),
        )),
        DataType::Int64 => Ok(Value::Number(Number::I(i128::from(
            array
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| type_error(data_type))?
                .value(row),
        )))),
        DataType::UInt64 => Ok(Value::Number(Number::U(u128::from(
            array
                .as_any()
                .downcast_ref::<UInt64Array>()
                .ok_or_else(|| type_error(data_type))?
                .value(row),
        )))),
        DataType::Float64 => Ok(Value::Number(Number::F64(
            array
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| type_error(data_type))?
                .value(row),
        ))),
        DataType::Binary => Ok(Value::Bytes(Bytes::new(
            array
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| type_error(data_type))?
                .value(row)
                .to_vec(),
        ))),
        DataType::Utf8 => Ok(Value::String(
            array
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| type_error(data_type))?
                .value(row)
                .to_owned(),
        )),
        other => Err(DataError::unsupported(format!(
            "Arrow type {other:?} is not mapped yet"
        ))),
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

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}
