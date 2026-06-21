use std::collections::HashMap;

use apache_avro::schema::{Name, Namespace, ResolvedSchema, Schema};
use arcweft_data::{DataError, DataErrorKind, DecodeBudget, DecodeLimits, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AvroTopLevel {
    Sequence,
    Scalar,
}

pub(crate) fn preflight_avro_container(
    input: &[u8],
    limits: &DecodeLimits,
    top_level: AvroTopLevel,
) -> Result<()> {
    let mut reader = SliceReader::new(input);
    reader.expect_bytes(b"Obj\x01")?;
    let metadata = read_metadata(&mut reader)?;
    reject_non_null_codec(&metadata)?;
    let writer_schema = parse_writer_schema(&metadata)?;
    let resolved = ResolvedSchema::try_from(&writer_schema).map_err(avro_error)?;
    reader.skip_exact(16)?;

    let mut budget = DecodeBudget::new(input.len(), limits)?;
    if top_level == AvroTopLevel::Sequence {
        budget.enter_node()?;
    }
    let result = scan_blocks(
        &mut reader,
        &writer_schema,
        resolved.get_names(),
        top_level,
        &mut budget,
    );
    if top_level == AvroTopLevel::Sequence {
        budget.exit_node();
    }
    result
}

fn scan_blocks(
    reader: &mut SliceReader<'_>,
    writer_schema: &Schema,
    names: &HashMap<Name, &Schema>,
    top_level: AvroTopLevel,
    budget: &mut DecodeBudget<'_>,
) -> Result<()> {
    let mut datums_seen = 0usize;
    while !reader.is_empty() {
        let count = read_nonnegative_count(reader, "Avro block datum count")?;
        let block_len = read_len(reader, "Avro block byte length")?;
        let block = reader.read_slice(block_len)?;
        reader.skip_exact(16)?;

        let mut block_reader = SliceReader::new(block);
        for _ in 0..count {
            datums_seen = datums_seen.saturating_add(1);
            if top_level == AvroTopLevel::Sequence {
                budget.sequence_item(datums_seen)?;
            }
            scan_value(&mut block_reader, writer_schema, names, &None, budget)?;
        }
        if !block_reader.is_empty() {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "Avro block contains trailing datum bytes",
            ));
        }
    }
    Ok(())
}

fn scan_value(
    reader: &mut SliceReader<'_>,
    schema: &Schema,
    names: &HashMap<Name, &Schema>,
    namespace: &Namespace,
    budget: &mut DecodeBudget<'_>,
) -> Result<()> {
    budget.enter_node()?;
    let result = scan_value_inner(reader, schema, names, namespace, budget);
    budget.exit_node();
    result
}

fn scan_value_inner(
    reader: &mut SliceReader<'_>,
    schema: &Schema,
    names: &HashMap<Name, &Schema>,
    namespace: &Namespace,
    budget: &mut DecodeBudget<'_>,
) -> Result<()> {
    match schema {
        Schema::Null => Ok(()),
        Schema::Boolean => reader.skip_exact(1),
        Schema::Int | Schema::Date | Schema::TimeMillis | Schema::Enum(_) => {
            read_zig_i64(reader).map(|_| ())
        }
        Schema::Long
        | Schema::TimeMicros
        | Schema::TimestampMillis
        | Schema::TimestampMicros
        | Schema::TimestampNanos
        | Schema::LocalTimestampMillis
        | Schema::LocalTimestampMicros
        | Schema::LocalTimestampNanos => read_zig_i64(reader).map(|_| ()),
        Schema::Float => reader.skip_exact(4),
        Schema::Double => reader.skip_exact(8),
        Schema::Bytes | Schema::BigDecimal => {
            let len = read_len(reader, "Avro bytes length")?;
            budget.bytes_len(len)?;
            reader.skip_exact(len)
        }
        Schema::String | Schema::Uuid => {
            let len = read_len(reader, "Avro string length")?;
            budget.string_len(len)?;
            reader.skip_exact(len)
        }
        Schema::Fixed(fixed) => {
            budget.bytes_len(fixed.size)?;
            reader.skip_exact(fixed.size)
        }
        Schema::Duration => reader.skip_exact(12),
        Schema::Decimal(decimal) => scan_value(reader, &decimal.inner, names, namespace, budget),
        Schema::Array(array) => scan_array(reader, &array.items, names, namespace, budget),
        Schema::Map(map) => scan_map(reader, &map.types, names, namespace, budget),
        Schema::Union(union) => {
            let index = read_zig_i64(reader)?;
            let index = usize::try_from(index).map_err(|_| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    format!("Avro union index {index} is negative or too large"),
                )
            })?;
            let variant = union.variants().get(index).ok_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    format!("Avro union index {index} is out of range"),
                )
            })?;
            scan_value(reader, variant, names, namespace, budget)
        }
        Schema::Record(record) => {
            budget.map_len(record.fields.len())?;
            let record_namespace = record.name.fully_qualified_name(namespace).namespace;
            record.fields.iter().try_for_each(|field| {
                scan_value(reader, &field.schema, names, &record_namespace, budget)
            })
        }
        Schema::Ref { name } => {
            let resolved_name = name.fully_qualified_name(namespace);
            let resolved = names.get(&resolved_name).ok_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    format!(
                        "unresolved Avro schema reference `{}`",
                        resolved_name.fullname(None)
                    ),
                )
            })?;
            scan_value(reader, resolved, names, &resolved_name.namespace, budget)
        }
    }
}

fn scan_array(
    reader: &mut SliceReader<'_>,
    item_schema: &Schema,
    names: &HashMap<Name, &Schema>,
    namespace: &Namespace,
    budget: &mut DecodeBudget<'_>,
) -> Result<()> {
    let mut len_after_item = 0usize;
    loop {
        let count = read_collection_block_count(reader, "Avro array block count")?;
        if count == 0 {
            return Ok(());
        }
        for _ in 0..count {
            len_after_item = len_after_item.saturating_add(1);
            budget.sequence_item(len_after_item)?;
            scan_value(reader, item_schema, names, namespace, budget)?;
        }
    }
}

fn scan_map(
    reader: &mut SliceReader<'_>,
    value_schema: &Schema,
    names: &HashMap<Name, &Schema>,
    namespace: &Namespace,
    budget: &mut DecodeBudget<'_>,
) -> Result<()> {
    let mut len_after_item = 0usize;
    loop {
        let count = read_collection_block_count(reader, "Avro map block count")?;
        if count == 0 {
            return Ok(());
        }
        for _ in 0..count {
            len_after_item = len_after_item.saturating_add(1);
            budget.map_item(len_after_item)?;
            let key_len = read_len(reader, "Avro map key length")?;
            budget.string_len(key_len)?;
            reader.skip_exact(key_len)?;
            scan_value(reader, value_schema, names, namespace, budget)?;
        }
    }
}

fn read_metadata(reader: &mut SliceReader<'_>) -> Result<HashMap<String, Vec<u8>>> {
    let mut metadata = HashMap::new();
    loop {
        let count = read_collection_block_count(reader, "Avro metadata block count")?;
        if count == 0 {
            return Ok(metadata);
        }
        for _ in 0..count {
            let key = read_string(reader, "Avro metadata key")?;
            let value_len = read_len(reader, "Avro metadata value length")?;
            let value = reader.read_slice(value_len)?.to_vec();
            metadata.insert(key, value);
        }
    }
}

fn reject_non_null_codec(metadata: &HashMap<String, Vec<u8>>) -> Result<()> {
    let Some(codec) = metadata.get("avro.codec") else {
        return Ok(());
    };
    if codec.as_slice() == b"null" {
        Ok(())
    } else {
        let label = std::str::from_utf8(codec).unwrap_or("<non-utf8>");
        Err(DataError::unsupported(format!(
            "compressed Avro codec `{label}` is not decoded under Arcweft limits"
        )))
    }
}

fn parse_writer_schema(metadata: &HashMap<String, Vec<u8>>) -> Result<Schema> {
    let schema = metadata.get("avro.schema").ok_or_else(|| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            "Avro container is missing avro.schema metadata",
        )
    })?;
    let schema = std::str::from_utf8(schema).map_err(|error| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("Avro schema metadata is not UTF-8: {error}"),
        )
    })?;
    Schema::parse_str(schema).map_err(avro_error)
}

fn read_collection_block_count(reader: &mut SliceReader<'_>, label: &str) -> Result<usize> {
    let raw = read_zig_i64(reader)?;
    if raw == 0 {
        return Ok(0);
    }
    let count = if raw < 0 {
        let block_size = read_zig_i64(reader)?;
        if block_size < 0 {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                format!("{label} byte size {block_size} is negative"),
            ));
        }
        raw.checked_neg().ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEncoding,
                format!("{label} overflows while negating {raw}"),
            )
        })?
    } else {
        raw
    };
    usize::try_from(count).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("{label} {count} is too large"),
        )
    })
}

fn read_nonnegative_count(reader: &mut SliceReader<'_>, label: &str) -> Result<usize> {
    let count = read_zig_i64(reader)?;
    if count < 0 {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("{label} {count} is negative"),
        ));
    }
    usize::try_from(count).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("{label} {count} is too large"),
        )
    })
}

fn read_len(reader: &mut SliceReader<'_>, label: &str) -> Result<usize> {
    let len = read_zig_i64(reader)?;
    if len < 0 {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("{label} {len} is negative"),
        ));
    }
    usize::try_from(len).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("{label} {len} is too large"),
        )
    })
}

fn read_string(reader: &mut SliceReader<'_>, label: &str) -> Result<String> {
    let len = read_len(reader, label)?;
    let bytes = reader.read_slice(len)?;
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|error| {
            DataError::new(
                DataErrorKind::InvalidEncoding,
                format!("{label} is not UTF-8: {error}"),
            )
        })
}

fn read_zig_i64(reader: &mut SliceReader<'_>) -> Result<i64> {
    let raw = read_var_u64(reader)?;
    let shifted = i64::try_from(raw >> 1).map_err(|_| {
        DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("Avro zigzag integer {raw} is too large"),
        )
    })?;
    Ok(if raw & 1 == 0 { shifted } else { !shifted })
}

fn read_var_u64(reader: &mut SliceReader<'_>) -> Result<u64> {
    let mut value = 0u64;
    for shift in (0..=63).step_by(7) {
        let byte = reader.read_byte()?;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(DataError::new(
        DataErrorKind::InvalidEncoding,
        "Avro variable integer is too large",
    ))
}

#[derive(Clone, Copy, Debug)]
struct SliceReader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> SliceReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn is_empty(&self) -> bool {
        self.offset == self.input.len()
    }

    fn read_byte(&mut self) -> Result<u8> {
        let byte = *self.input.get(self.offset).ok_or_else(unexpected_eof)?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.offset.checked_add(len).ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEncoding,
                format!("Avro byte range length {len} overflows"),
            )
        })?;
        let slice = self
            .input
            .get(self.offset..end)
            .ok_or_else(unexpected_eof)?;
        self.offset = end;
        Ok(slice)
    }

    fn skip_exact(&mut self, len: usize) -> Result<()> {
        self.read_slice(len).map(|_| ())
    }

    fn expect_bytes(&mut self, expected: &[u8]) -> Result<()> {
        let actual = self.read_slice(expected.len())?;
        if actual == expected {
            Ok(())
        } else {
            Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "Avro container header magic is invalid",
            ))
        }
    }
}

fn unexpected_eof() -> DataError {
    DataError::new(
        DataErrorKind::InvalidEncoding,
        "unexpected end of Avro container",
    )
}

fn avro_error(error: impl std::fmt::Display) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
}
