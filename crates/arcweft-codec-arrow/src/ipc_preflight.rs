use std::collections::BTreeSet;

use arcweft_data::{DataError, DataErrorKind, DecodeBudget, DecodeLimits, Result};
use arrow::datatypes::DataType;
use arrow::ipc::convert::fb_to_schema;
use arrow::ipc::reader::read_footer_length;
use arrow::ipc::{Block, MessageHeader, root_as_footer, root_as_message};

use crate::{ArrowRowShape, arrow_data_type};

const CONTINUATION_MARKER: [u8; 4] = [0xff; 4];

pub(crate) fn preflight_arrow_ipc_buffers(
    input: &[u8],
    row_shape: ArrowRowShape<'_>,
    limits: &DecodeLimits,
) -> Result<()> {
    let mut budget = DecodeBudget::new(input.len(), limits)?;
    budget.enter_node()?;
    let footer = arrow_footer(input)?;
    let schema = footer.schema().map(fb_to_schema).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC footer has no schema",
        )
    })?;
    reject_unknown_schema_fields(&schema, row_shape)?;
    if footer
        .dictionaries()
        .is_some_and(|dictionaries| !dictionaries.is_empty())
    {
        return Err(DataError::unsupported(
            "Arrow IPC dictionaries are not supported by Arcweft data preflight",
        ));
    }
    for block in footer.recordBatches().into_iter().flatten() {
        preflight_arrow_block(input, block, row_shape, &schema, &mut budget)?;
    }
    budget.exit_node();
    Ok(())
}

fn arrow_footer(input: &[u8]) -> Result<arrow::ipc::Footer<'_>> {
    let trailer_start = input.len().checked_sub(10).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC file is too short",
        )
    })?;
    let trailer = input[trailer_start..].try_into().map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC trailer is invalid",
        )
    })?;
    let footer_len = read_footer_length(trailer).map_err(arrow_error)?;
    let footer_start = trailer_start.checked_sub(footer_len).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC footer length exceeds file length",
        )
    })?;
    root_as_footer(&input[footer_start..trailer_start]).map_err(arrow_error)
}

fn reject_unknown_schema_fields(
    schema: &arrow::datatypes::Schema,
    row_shape: ArrowRowShape<'_>,
) -> Result<()> {
    if !row_shape.policy.deny_unknown_fields {
        return Ok(());
    }
    let known = row_shape
        .fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| field.wire_name.as_str())
        .collect::<BTreeSet<_>>();
    schema
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

fn preflight_arrow_block(
    input: &[u8],
    block: &Block,
    row_shape: ArrowRowShape<'_>,
    schema: &arrow::datatypes::Schema,
    budget: &mut DecodeBudget<'_>,
) -> Result<()> {
    let metadata = block_metadata(input, block)?;
    let body = block_body(input, block)?;
    let message = parse_message(metadata)?;
    let batch = message.header_as_record_batch().ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC block is not a record batch",
        )
    })?;
    if batch.compression().is_some() {
        return Err(DataError::unsupported(
            "compressed Arrow IPC record batches are not supported by Arcweft data preflight",
        ));
    }
    let row_count = usize::try_from(batch.length()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "Arrow IPC record batch length {} is invalid",
                batch.length()
            ),
        )
    })?;
    budget.sequence_len(row_count)?;
    let buffers = batch.buffers().ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC record batch has no buffer metadata",
        )
    })?;
    let mut buffer_index = 0_usize;
    for field in schema.fields() {
        let Some(data_type) = expected_field_type(row_shape, field.name()) else {
            buffer_index = buffer_index.saturating_add(arrow_buffer_count(field.data_type()));
            continue;
        };
        match (field.data_type(), data_type) {
            (DataType::Utf8, DataType::Utf8) => {
                preflight_variable_width_field(
                    body,
                    *buffers.get(buffer_index + 1),
                    *buffers.get(buffer_index + 2),
                    row_count,
                    budget,
                    VariableWidthBudget::String,
                )?;
                buffer_index += 3;
            }
            (DataType::Binary, DataType::Binary) => {
                preflight_variable_width_field(
                    body,
                    *buffers.get(buffer_index + 1),
                    *buffers.get(buffer_index + 2),
                    row_count,
                    budget,
                    VariableWidthBudget::Bytes,
                )?;
                buffer_index += 3;
            }
            _ => {
                buffer_index = buffer_index.saturating_add(arrow_buffer_count(field.data_type()));
            }
        }
    }
    Ok(())
}

fn expected_field_type(row_shape: ArrowRowShape<'_>, wire_name: &str) -> Option<DataType> {
    row_shape
        .fields
        .iter()
        .filter(|field| !field.skip)
        .find(|field| field.wire_name == wire_name)
        .and_then(|field| arrow_data_type(&field.value_shape()).ok())
}

fn arrow_buffer_count(data_type: &DataType) -> usize {
    match data_type {
        DataType::Null => 0,
        DataType::Utf8 | DataType::Binary => 3,
        _ => 2,
    }
}

#[derive(Clone, Copy)]
enum VariableWidthBudget {
    String,
    Bytes,
}

fn preflight_variable_width_field(
    body: &[u8],
    offsets: arrow::ipc::Buffer,
    values: arrow::ipc::Buffer,
    row_count: usize,
    budget: &DecodeBudget<'_>,
    budget_kind: VariableWidthBudget,
) -> Result<()> {
    let offsets = body_buffer(body, &offsets)?;
    let values_len = usize::try_from(values.length()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "Arrow IPC value buffer length {} is invalid",
                values.length()
            ),
        )
    })?;
    let needed_offsets = row_count
        .checked_add(1)
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| DataError::limit("Arrow IPC offset buffer length overflow"))?;
    if offsets.len() < needed_offsets {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC offset buffer is shorter than the record batch row count",
        ));
    }
    let mut previous = read_i32_offset(offsets, 0)?;
    for index in 1..=row_count {
        let current = read_i32_offset(offsets, index)?;
        if current < previous || current > values_len {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "Arrow IPC variable-width offsets are invalid",
            ));
        }
        let len = current - previous;
        match budget_kind {
            VariableWidthBudget::String => budget.string_len(len)?,
            VariableWidthBudget::Bytes => budget.bytes_len(len)?,
        }
        previous = current;
    }
    Ok(())
}

fn read_i32_offset(offsets: &[u8], index: usize) -> Result<usize> {
    let start = index
        .checked_mul(4)
        .ok_or_else(|| DataError::limit("Arrow IPC offset index overflow"))?;
    let bytes = offsets.get(start..start + 4).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC offset is missing",
        )
    })?;
    let offset = i32::from_le_bytes(bytes.try_into().expect("slice length checked"));
    usize::try_from(offset).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("Arrow IPC offset {offset} is negative"),
        )
    })
}

fn body_buffer<'a>(body: &'a [u8], buffer: &arrow::ipc::Buffer) -> Result<&'a [u8]> {
    let offset = usize::try_from(buffer.offset()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("Arrow IPC buffer offset {} is invalid", buffer.offset()),
        )
    })?;
    let len = usize::try_from(buffer.length()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("Arrow IPC buffer length {} is invalid", buffer.length()),
        )
    })?;
    let end = offset.checked_add(len).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC buffer range overflow",
        )
    })?;
    body.get(offset..end).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC buffer range exceeds record batch body",
        )
    })
}

fn block_metadata<'a>(input: &'a [u8], block: &Block) -> Result<&'a [u8]> {
    let offset = block_offset(block)?;
    let len = usize::try_from(block.metaDataLength()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "Arrow IPC block metadata length {} is invalid",
                block.metaDataLength()
            ),
        )
    })?;
    slice_range(input, offset, len)
}

fn block_body<'a>(input: &'a [u8], block: &Block) -> Result<&'a [u8]> {
    let metadata_len = usize::try_from(block.metaDataLength()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "Arrow IPC block metadata length {} is invalid",
                block.metaDataLength()
            ),
        )
    })?;
    let body_len = usize::try_from(block.bodyLength()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "Arrow IPC block body length {} is invalid",
                block.bodyLength()
            ),
        )
    })?;
    let offset = block_offset(block)?
        .checked_add(metadata_len)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEncoding,
                "Arrow IPC body offset overflow",
            )
        })?;
    slice_range(input, offset, body_len)
}

fn block_offset(block: &Block) -> Result<usize> {
    usize::try_from(block.offset()).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("Arrow IPC block offset {} is invalid", block.offset()),
        )
    })
}

fn slice_range(input: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset.checked_add(len).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC block range overflow",
        )
    })?;
    input.get(offset..end).ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC block range exceeds file length",
        )
    })
}

fn parse_message(metadata: &[u8]) -> Result<arrow::ipc::Message<'_>> {
    let data = if metadata.starts_with(&CONTINUATION_MARKER) {
        metadata.get(8..)
    } else {
        metadata.get(4..)
    }
    .ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC message is too short",
        )
    })?;
    let message = root_as_message(data).map_err(arrow_error)?;
    if message.header_type() != MessageHeader::RecordBatch {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "Arrow IPC message is not a record batch",
        ));
    }
    Ok(message)
}

fn arrow_error(error: impl std::fmt::Display) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
}
