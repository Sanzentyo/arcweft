use std::sync::Arc;

use arcweft_data::{DataError, DataErrorKind, DecodeLimits, FieldShape, Result, TypeShape};
use bytes::Bytes as ByteBuffer;
use parquet::basic::{Compression, Type as PhysicalType};
use parquet::column::page::PageReader;
use parquet::file::metadata::{ColumnChunkMetaData, ParquetMetaData, RowGroupMetaData};
use parquet::file::reader::SerializedPageReader;

use crate::ArrowRowShape;

pub(crate) fn preflight_parquet_buffers(
    input: &ByteBuffer,
    metadata: &ParquetMetaData,
    row_shape: ArrowRowShape<'_>,
    limits: &DecodeLimits,
) -> Result<()> {
    let columns = VariableWidthColumns::from_fields(row_shape.fields, limits);
    if columns.is_empty() {
        return Ok(());
    }

    let input = Arc::new(input.clone());
    for row_group in metadata.row_groups() {
        preflight_row_group(&input, row_group, &columns)?;
    }
    Ok(())
}

fn preflight_row_group(
    input: &Arc<ByteBuffer>,
    row_group: &RowGroupMetaData,
    columns: &VariableWidthColumns,
) -> Result<()> {
    let total_rows = usize::try_from(row_group.num_rows()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "Parquet row group has invalid row count {}",
                row_group.num_rows()
            ),
        )
    })?;
    for column in row_group.columns() {
        let Some(limit) = columns.limit_for(column) else {
            continue;
        };
        preflight_column_chunk_metadata(column, limit)?;
        let mut reader = SerializedPageReader::new(input.clone(), column, total_rows, None)
            .map_err(parquet_error)?;
        while let Some(page) = reader.get_next_page().map_err(parquet_error)? {
            let page_len = page.buffer().len();
            if page_len > limit {
                return Err(DataError::limit(format!(
                    "Parquet variable-width page buffer length {page_len} exceeds {limit}"
                )));
            }
        }
    }
    Ok(())
}

fn preflight_column_chunk_metadata(column: &ColumnChunkMetaData, limit: usize) -> Result<()> {
    if column.compression() != Compression::UNCOMPRESSED {
        return Err(DataError::unsupported(format!(
            "compressed Parquet variable-width column {} is not decoded under Arcweft limits",
            column.column_path().string()
        )));
    }

    reject_negative_size("Parquet compressed column chunk", column.compressed_size())?;
    reject_negative_size(
        "Parquet uncompressed column chunk",
        column.uncompressed_size(),
    )?;

    let uncompressed_size = usize::try_from(column.uncompressed_size()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "Parquet uncompressed column chunk size {} is invalid",
                column.uncompressed_size()
            ),
        )
    })?;
    if uncompressed_size > limit {
        return Err(DataError::limit(format!(
            "Parquet variable-width column chunk uncompressed size {uncompressed_size} exceeds {limit}"
        )));
    }

    if let Some(unencoded_size) = column.unencoded_byte_array_data_bytes() {
        reject_negative_size("Parquet unencoded byte-array data", unencoded_size)?;
        let unencoded_size = usize::try_from(unencoded_size).map_err(|_| {
            DataError::new(
                DataErrorKind::InvalidEncoding,
                format!("Parquet unencoded byte-array data size {unencoded_size} is invalid"),
            )
        })?;
        if unencoded_size > limit {
            return Err(DataError::limit(format!(
                "Parquet unencoded byte-array data length {unencoded_size} exceeds {limit}"
            )));
        }
    }

    Ok(())
}

fn reject_negative_size(label: &str, value: i64) -> Result<()> {
    if value < 0 {
        Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("{label} size {value} is negative"),
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
struct VariableWidthColumns<'a> {
    entries: Vec<VariableWidthColumn<'a>>,
}

impl<'a> VariableWidthColumns<'a> {
    fn from_fields(fields: &'a [FieldShape], limits: &DecodeLimits) -> Self {
        Self {
            entries: fields
                .iter()
                .filter(|field| !field.skip)
                .filter_map(|field| VariableWidthColumn::from_field(field, limits))
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn limit_for(&self, column: &ColumnChunkMetaData) -> Option<usize> {
        if !matches!(
            column.column_type(),
            PhysicalType::BYTE_ARRAY | PhysicalType::FIXED_LEN_BYTE_ARRAY
        ) {
            return None;
        }

        let path = column.column_path().parts();
        self.entries
            .iter()
            .find(|entry| path.len() == 1 && path[0] == entry.name)
            .map(|entry| entry.limit)
    }
}

#[derive(Debug)]
struct VariableWidthColumn<'a> {
    name: &'a str,
    limit: usize,
}

impl<'a> VariableWidthColumn<'a> {
    fn from_field(field: &'a FieldShape, limits: &DecodeLimits) -> Option<Self> {
        let limit = match option_inner(&field.value_shape()) {
            TypeShape::String | TypeShape::Char => limits.max_string_len,
            TypeShape::Bytes { .. } => limits.max_bytes_len,
            _ => return None,
        };
        Some(Self {
            name: field.wire_name.as_str(),
            limit,
        })
    }
}

fn option_inner(shape: &TypeShape) -> &TypeShape {
    match shape {
        TypeShape::Option(inner) => inner,
        other => other,
    }
}

fn parquet_error(error: impl std::fmt::Display) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
}
