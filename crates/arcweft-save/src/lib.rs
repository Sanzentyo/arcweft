#![forbid(unsafe_code)]

use arcweft_data::{
    CodecRegistry, DataError, DataErrorKind, DecodeOptions, EncodeOptions, Result, TypeShape, Value,
};

const MAGIC: &[u8; 8] = b"AWFS\0\0\0\x01";
const HEADER_LEN: usize = MAGIC.len() + 4 + 4 + 4 + 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveSchemaId(String);

impl SaveSchemaId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveEnvelope {
    pub schema_id: SaveSchemaId,
    pub schema_version: u32,
    pub codec_id: String,
    pub payload_checksum: [u8; 32],
    pub payload: Vec<u8>,
}

impl SaveEnvelope {
    #[must_use]
    pub fn new(
        schema_id: SaveSchemaId,
        schema_version: u32,
        codec_id: impl Into<String>,
        payload: Vec<u8>,
    ) -> Self {
        let payload_checksum = *blake3::hash(&payload).as_bytes();
        Self {
            schema_id,
            schema_version,
            codec_id: codec_id.into(),
            payload_checksum,
            payload,
        }
    }

    pub fn encode_bytes(&self) -> Result<Vec<u8>> {
        let schema = self.schema_id.as_str().as_bytes();
        let codec = self.codec_id.as_bytes();
        let schema_len = u32::try_from(schema.len())
            .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "schema id too long"))?;
        let codec_len = u32::try_from(codec.len())
            .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "codec id too long"))?;
        let payload_len = u32::try_from(self.payload.len())
            .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "payload too large"))?;
        let mut out =
            Vec::with_capacity(HEADER_LEN + schema.len() + codec.len() + self.payload.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&schema_len.to_le_bytes());
        out.extend_from_slice(&codec_len.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&self.payload_checksum);
        out.extend_from_slice(&self.schema_version.to_le_bytes());
        out.extend_from_slice(schema);
        out.extend_from_slice(codec);
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    pub fn decode_bytes(input: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(input);
        cursor.expect(MAGIC)?;
        let schema_len = cursor.u32()? as usize;
        let codec_len = cursor.u32()? as usize;
        let payload_len = cursor.u32()? as usize;
        let checksum = cursor.array::<32>()?;
        let schema_version = cursor.u32()?;
        let schema_id = String::from_utf8(cursor.bytes(schema_len)?.to_vec())
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let codec_id = String::from_utf8(cursor.bytes(codec_len)?.to_vec())
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let payload = cursor.bytes(payload_len)?.to_vec();
        if *blake3::hash(&payload).as_bytes() != checksum {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "save payload checksum mismatch",
            ));
        }
        Ok(Self {
            schema_id: SaveSchemaId::new(schema_id),
            schema_version,
            codec_id,
            payload_checksum: checksum,
            payload,
        })
    }
}

pub trait SaveMigration {
    fn schema_id(&self) -> &SaveSchemaId;

    fn current_version(&self) -> u32;

    fn migrate(&self, from_version: u32, value: Value) -> Result<Value>;
}

pub fn encode_save(
    value: &Value,
    shape: &TypeShape,
    schema_id: SaveSchemaId,
    schema_version: u32,
    codec_id: &str,
    registry: &CodecRegistry,
) -> Result<Vec<u8>> {
    let codec = registry.by_id(codec_id)?;
    let payload = codec.encode_value(value, shape, &EncodeOptions::default())?;
    SaveEnvelope::new(schema_id, schema_version, codec_id, payload).encode_bytes()
}

pub fn decode_save(
    input: &[u8],
    shape: &TypeShape,
    registry: &CodecRegistry,
    migration: Option<&dyn SaveMigration>,
) -> Result<Value> {
    let envelope = SaveEnvelope::decode_bytes(input)?;
    let codec = registry.by_id(&envelope.codec_id)?;
    let value = codec.decode_value(&envelope.payload, shape, &DecodeOptions::default())?;
    match migration {
        Some(migration) if envelope.schema_version != migration.current_version() => {
            migration.migrate(envelope.schema_version, value)
        }
        _ => Ok(value),
    }
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn expect(&mut self, expected: &[u8]) -> Result<()> {
        let bytes = self.bytes(expected.len())?;
        if bytes == expected {
            Ok(())
        } else {
            Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "bad save magic",
            ))
        }
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.bytes(N)?;
        let mut out = [0; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| DataError::new(DataErrorKind::InvalidEncoding, "offset overflow"))?;
        if end > self.input.len() {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "unexpected end of save envelope",
            ));
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
}
