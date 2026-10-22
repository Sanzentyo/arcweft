use apache_avro::{Schema, schema::RecordField};
use arcweft_data::{DataError, DataErrorKind, Result};
use serde::Serialize;

use super::{
    AvroDatum, MeasuredOutput, Output, invalid_encoding, write_bytes, write_length, write_long,
};

/// A single schema-directed datum. Consuming methods prevent writing two values
/// in a field; records and arrays check the complete field/item grammar.
pub struct DatumWriter<'a> {
    schema: &'a Schema,
    output: &'a mut dyn Output,
}

impl<'a> DatumWriter<'a> {
    pub(super) fn new(schema: &'a Schema, output: &'a mut dyn Output) -> Self {
        Self { schema, output }
    }

    pub fn schema(&self) -> &'a Schema {
        self.schema
    }

    pub fn null(self) -> Result<()> {
        self.require(matches!(self.schema, Schema::Null), "null")
    }

    pub fn boolean(self, value: bool) -> Result<()> {
        self.require(matches!(self.schema, Schema::Boolean), "boolean")?;
        self.output.append(&[u8::from(value)])
    }

    pub fn integer(self, value: i64) -> Result<()> {
        match self.schema {
            Schema::Int => {
                i32::try_from(value).map_err(|_| {
                    DataError::new(DataErrorKind::NumberOutOfRange, "Avro int is i32-limited")
                })?;
            }
            Schema::Long => {}
            _ => return Err(invalid_encoding("expected Avro int or long")),
        }
        write_long(self.output, value)
    }

    pub fn float(self, value: f32) -> Result<()> {
        self.require(matches!(self.schema, Schema::Float), "float")?;
        self.require(value.is_finite(), "finite float")?;
        self.output.append(&value.to_le_bytes())
    }

    pub fn double(self, value: f64) -> Result<()> {
        self.require(matches!(self.schema, Schema::Double), "double")?;
        self.require(value.is_finite(), "finite double")?;
        self.output.append(&value.to_le_bytes())
    }

    pub fn string(self, value: &str) -> Result<()> {
        self.require(matches!(self.schema, Schema::String), "string")?;
        write_bytes(self.output, value.as_bytes())
    }

    /// Writes JSON as a UTF-8 Avro string, measuring it without constructing an
    /// intermediate String or Value. Serialization errors retain their cause.
    pub fn json_string<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        self.require(matches!(self.schema, Schema::String), "string")?;
        let mut measured = MeasuredOutput::new(self.output.remaining_len()?, None);
        write_json(&mut measured, value)?;
        write_length(self.output, measured.length)?;
        let mut written = MeasuredOutput::new(measured.length, Some(self.output));
        write_json(&mut written, value)?;
        if written.length != measured.length
            || written.hasher.finalize() != measured.hasher.finalize()
        {
            return Err(invalid_encoding(
                "Avro JSON string changed between encoding passes",
            ));
        }
        Ok(())
    }

    pub fn bytes(self, value: &[u8]) -> Result<()> {
        match self.schema {
            Schema::Bytes => write_bytes(self.output, value),
            Schema::Fixed(fixed) if fixed.size == value.len() => self.output.append(value),
            Schema::Fixed(fixed) => Err(invalid_encoding(format!(
                "fixed Avro field requires {} bytes, found {}",
                fixed.size,
                value.len()
            ))),
            _ => Err(invalid_encoding("expected Avro bytes or fixed")),
        }
    }

    pub fn enumeration(self, symbol: &str) -> Result<()> {
        let Schema::Enum(schema) = self.schema else {
            return Err(invalid_encoding("expected Avro enum"));
        };
        let index = schema
            .symbols
            .iter()
            .position(|candidate| candidate == symbol)
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEnumTag,
                    format!("unknown Avro enum symbol `{symbol}`"),
                )
            })?;
        let index =
            i32::try_from(index).map_err(|_| DataError::limit("Avro enum index exceeds i32"))?;
        write_long(self.output, i64::from(index))
    }

    pub fn union(
        self,
        index: usize,
        value: impl FnOnce(DatumWriter<'a>) -> Result<()>,
    ) -> Result<()> {
        let Schema::Union(union) = self.schema else {
            return Err(invalid_encoding("expected Avro union"));
        };
        let schema = union
            .variants()
            .get(index)
            .ok_or_else(|| invalid_encoding("Avro union branch index out of range"))?;
        write_length(self.output, index)?;
        value(DatumWriter::new(schema, self.output))
    }

    pub fn array<I>(self, values: I) -> Result<()>
    where
        I: ExactSizeIterator,
        I::Item: AvroDatum,
    {
        let Schema::Array(array) = self.schema else {
            return Err(invalid_encoding("expected Avro array"));
        };
        let count = values.len();
        if count != 0 {
            write_length(self.output, count)?;
        }
        let mut actual = 0usize;
        for (index, value) in values.enumerate() {
            if index >= count {
                return Err(invalid_encoding("Avro array iterator length changed"));
            }
            value
                .write_avro(DatumWriter::new(&array.items, self.output))
                .map_err(|error| error.at_index(index))?;
            actual += 1;
        }
        if actual != count {
            return Err(invalid_encoding("Avro array iterator length changed"));
        }
        write_long(self.output, 0)
    }

    pub fn record(self, write: impl FnOnce(&mut RecordWriter<'_>) -> Result<()>) -> Result<()> {
        let Schema::Record(record) = self.schema else {
            return Err(invalid_encoding("expected Avro record"));
        };
        let mut writer = RecordWriter {
            fields: &record.fields,
            next: 0,
            output: self.output,
        };
        write(&mut writer)?;
        if writer.next != writer.fields.len() {
            return Err(invalid_encoding("Avro record has missing fields"));
        }
        Ok(())
    }

    fn require(&self, condition: bool, expected: &str) -> Result<()> {
        if condition {
            Ok(())
        } else {
            Err(invalid_encoding(format!(
                "expected Avro {expected}, found {:?}",
                apache_avro::schema::SchemaKind::from(self.schema)
            )))
        }
    }
}

/// A record's schema order is authoritative even when the source uses a map or
/// a different reflection order. No out-of-order field buffering is needed.
pub struct RecordWriter<'a> {
    fields: &'a [RecordField],
    next: usize,
    output: &'a mut dyn Output,
}

impl RecordWriter<'_> {
    pub fn field(
        &mut self,
        name: &str,
        value: impl FnOnce(DatumWriter<'_>) -> Result<()>,
    ) -> Result<()> {
        let field = self
            .fields
            .get(self.next)
            .filter(|field| field.name == name)
            .ok_or_else(|| invalid_encoding(format!("unexpected Avro record field `{name}`")))?;
        value(DatumWriter::new(&field.schema, self.output))
            .map_err(|error| error.at_field(name.to_owned()))?;
        self.next += 1;
        Ok(())
    }
}

fn write_json<T: Serialize + ?Sized>(output: &mut dyn Output, value: &T) -> Result<()> {
    struct JsonOutput<'a> {
        output: &'a mut dyn Output,
        failure: Option<DataError>,
    }
    impl std::io::Write for JsonOutput<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Some(error) = &self.failure {
                return Err(std::io::Error::other(error.clone()));
            }
            if let Err(error) = self.output.append(bytes) {
                self.failure = Some(error.clone());
                return Err(std::io::Error::other(error));
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = JsonOutput {
        output,
        failure: None,
    };
    let result = serde_json::to_writer(&mut writer, value);
    if let Some(error) = writer.failure {
        return Err(error);
    }
    result.map_err(invalid_encoding)
}
