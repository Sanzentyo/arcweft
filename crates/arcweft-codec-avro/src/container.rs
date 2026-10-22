//! Deterministic, uncompressed Avro Object Container output. The exact admitted
//! schema text is metadata; metadata order and block framing have one owner.
//! The version-1 Arcweft profile derives the 128-bit sync marker from the schema,
//! record count and datum bytes, instead of drawing fresh randomness per write.

use apache_avro::Schema;
use arcweft_data::{DataError, DataErrorKind, EncodeBuffer, Result};

mod datum;
pub use datum::{DatumWriter, RecordWriter};

/// A borrowed datum writes against the schema supplied by its container.
/// Implementations must produce the same bytes on repeated calls. The container
/// checks both length and digest before publishing the second pass.
pub trait AvroDatum {
    fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()>;
}

impl<T: AvroDatum + ?Sized> AvroDatum for &T {
    fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
        T::write_avro(self, writer)
    }
}

/// An admitted Avro schema and its exact source, shared by data and bundle
/// writers. Encoding does not clone a schema, datum tree, JSON payload or block.
#[derive(Clone, Debug)]
pub struct ObjectContainer {
    schema: Schema,
    schema_json: Box<str>,
}

impl ObjectContainer {
    pub fn new(schema_json: &str) -> Result<Self> {
        let schema = Schema::parse_str(schema_json).map_err(invalid_encoding)?;
        Ok(Self {
            schema,
            schema_json: schema_json.into(),
        })
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Measures borrowed records under the remaining output budget, derives the
    /// sync marker, then writes the same records directly to the bounded buffer.
    /// Empty input writes a header; nonempty input writes one null-codec block.
    pub fn encode_to<I>(&self, records: impl Fn() -> I, output: &mut EncodeBuffer) -> Result<()>
    where
        I: ExactSizeIterator,
        I::Item: AvroDatum,
    {
        let measured_records = records();
        let count = measured_records.len();
        let count = i64::try_from(count).map_err(|_| DataError::limit("Avro record count"))?;
        let mut header = MeasuredOutput::new(output.remaining_len()?, None);
        self.write_header_prefix(&mut header)?;
        let header_len = header.length;
        // Reserve all known framing before traversing even a zero-byte datum.
        header.append(&[0; 16])?;
        if count != 0 {
            write_long(&mut header, count)?;
            header.append(&[0; 16])?;
        }
        let available = header.remaining_len()?;
        let mut measured = MeasuredOutput::new(available, None);
        measured.hasher.update(b"arcweft.avro.object-container.1\0");
        measured
            .hasher
            .update(&(self.schema_json.len() as u64).to_le_bytes());
        measured.hasher.update(self.schema_json.as_bytes());
        measured.hasher.update(&count.to_le_bytes());
        let seed = measured.hasher.clone();
        self.write_records(measured_records, count, &mut measured)?;
        let datum_len = measured.length;
        let digest = measured.hasher.finalize();
        let marker = &digest.as_bytes()[..16];
        if count != 0 {
            // The block byte-count prefix is known only after measurement.
            let mut prefix = MeasuredOutput::new(available - datum_len, None);
            write_length(&mut prefix, datum_len)?;
        }

        // No encoded bytes were allocated before the complete length check.
        {
            let mut written = MeasuredOutput::new(output.remaining_len()?, Some(output));
            self.write_header_prefix(&mut written)?;
            debug_assert_eq!(written.length, header_len);
            written.append(marker)?;
            if count != 0 {
                write_long(&mut written, count)?;
                write_length(&mut written, datum_len)?;
                let mut data = MeasuredOutput::new(datum_len, written.output.take());
                data.hasher = seed;
                self.write_records(records(), count, &mut data)?;
                if data.length != datum_len || data.hasher.finalize() != digest {
                    return Err(invalid_encoding(
                        "Avro datum changed between encoding passes",
                    ));
                }
            }
        }
        if count != 0 {
            // All datum borrows have ended; framing uses the original buffer.
            output.append(marker)?;
        }
        Ok(())
    }

    fn write_header_prefix(&self, output: &mut dyn Output) -> Result<()> {
        output.append(b"Obj\x01")?;
        write_long(output, 2)?;
        write_bytes(output, b"avro.codec")?;
        write_bytes(output, b"null")?;
        write_bytes(output, b"avro.schema")?;
        write_bytes(output, self.schema_json.as_bytes())?;
        write_long(output, 0)
    }

    fn write_records<I>(&self, records: I, expected: i64, output: &mut dyn Output) -> Result<()>
    where
        I: Iterator,
        I::Item: AvroDatum,
    {
        let mut count = 0i64;
        for record in records {
            if count >= expected {
                return Err(invalid_encoding("Avro record iterator length changed"));
            }
            record.write_avro(DatumWriter::new(&self.schema, output))?;
            count += 1;
        }
        if count != expected {
            return Err(invalid_encoding("Avro record iterator length changed"));
        }
        Ok(())
    }
}

pub(super) trait Output {
    fn append(&mut self, bytes: &[u8]) -> Result<()>;
    fn remaining_len(&self) -> Result<usize>;
}

impl Output for EncodeBuffer {
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        EncodeBuffer::append(self, bytes)
    }
    fn remaining_len(&self) -> Result<usize> {
        EncodeBuffer::remaining_len(self)
    }
}

struct MeasuredOutput<'a> {
    output: Option<&'a mut dyn Output>,
    maximum: usize,
    length: usize,
    hasher: blake3::Hasher,
}

impl<'a> MeasuredOutput<'a> {
    fn new(maximum: usize, output: Option<&'a mut dyn Output>) -> Self {
        Self {
            output,
            maximum,
            length: 0,
            hasher: blake3::Hasher::new(),
        }
    }
}

impl Output for MeasuredOutput<'_> {
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        let length = self
            .length
            .checked_add(bytes.len())
            .filter(|length| *length <= self.maximum)
            .ok_or_else(|| {
                DataError::limit("Avro encoded output length exceeds remaining budget")
            })?;
        if let Some(buffer) = &mut self.output {
            buffer.append(bytes)?;
        }
        self.hasher.update(bytes);
        self.length = length;
        Ok(())
    }

    fn remaining_len(&self) -> Result<usize> {
        Ok(self.maximum - self.length)
    }
}

pub(super) fn write_long(output: &mut dyn Output, value: i64) -> Result<()> {
    let mut value = (value.cast_unsigned() << 1) ^ (value >> 63).cast_unsigned();
    let mut bytes = [0; 10];
    let mut length = 0;
    loop {
        bytes[length] = (value.to_le_bytes()[0] & 0x7f) | if value > 0x7f { 0x80 } else { 0 };
        length += 1;
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    output.append(&bytes[..length])
}

pub(super) fn write_length(output: &mut dyn Output, length: usize) -> Result<()> {
    write_long(
        output,
        i64::try_from(length).map_err(|_| DataError::limit("Avro length exceeds i64"))?,
    )
}

fn write_bytes(output: &mut dyn Output, bytes: &[u8]) -> Result<()> {
    write_length(output, bytes.len())?;
    output.append(bytes)
}

fn invalid_encoding(error: impl std::fmt::Display) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
}
