#![forbid(unsafe_code)]

use arcweft_data::{
    CodecRegistry, DataError, DataErrorKind, DecodeOptions, EncodeOptions, Result, TypeShape,
    Value, encode_with_shape,
};

const MAGIC: &[u8; 8] = b"AWFS\0\0\0\x01";
const HEADER_LEN: usize = MAGIC.len() + 4 + 4 + 4 + 32 + 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveDecodeOptions {
    pub max_envelope_bytes: u64,
    pub max_schema_id_bytes: u32,
    pub max_codec_id_bytes: u32,
    pub max_payload_bytes: u64,
    pub allow_trailing_data: bool,
    pub codec: DecodeOptions,
}

impl Default for SaveDecodeOptions {
    fn default() -> Self {
        Self {
            max_envelope_bytes: 512 * 1024 * 1024,
            max_schema_id_bytes: 1024,
            max_codec_id_bytes: 256,
            max_payload_bytes: 256 * 1024 * 1024,
            allow_trailing_data: false,
            codec: DecodeOptions::default(),
        }
    }
}

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

    pub fn decode_bytes(input: &[u8], options: &SaveDecodeOptions) -> Result<Self> {
        if u64::try_from(input.len()).is_ok_and(|len| len > options.max_envelope_bytes) {
            return Err(DataError::limit(format!(
                "save envelope length {} exceeds {}",
                input.len(),
                options.max_envelope_bytes
            )));
        }
        let mut cursor = Cursor::new(input);
        cursor.expect(MAGIC)?;
        let schema_len = cursor.bounded_len("schema id", u64::from(options.max_schema_id_bytes))?;
        let codec_len = cursor.bounded_len("codec id", u64::from(options.max_codec_id_bytes))?;
        let payload_len = cursor.bounded_len("payload", options.max_payload_bytes)?;
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
        cursor.finish(options.allow_trailing_data)?;
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

pub trait SaveMigrationStep {
    fn schema_id(&self) -> &SaveSchemaId;

    fn source_version(&self) -> u32;

    fn target_version(&self) -> u32;

    fn migrate(&self, value: Value) -> Result<Value>;
}

pub struct SaveMigrationChain<'a> {
    schema_id: SaveSchemaId,
    current_version: u32,
    steps: Vec<&'a dyn SaveMigrationStep>,
}

impl<'a> SaveMigrationChain<'a> {
    pub fn new(
        schema_id: SaveSchemaId,
        current_version: u32,
        steps: impl IntoIterator<Item = &'a dyn SaveMigrationStep>,
    ) -> Result<Self> {
        let steps = steps.into_iter().collect::<Vec<_>>();
        validate_migration_steps(&schema_id, current_version, &steps)?;
        Ok(Self {
            schema_id,
            current_version,
            steps,
        })
    }

    fn step_from(&self, version: u32) -> Result<&dyn SaveMigrationStep> {
        self.steps
            .iter()
            .copied()
            .find(|step| step.source_version() == version)
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    format!(
                        "save migration chain has no step from version {version} to {}",
                        self.current_version
                    ),
                )
            })
    }
}

impl SaveMigration for SaveMigrationChain<'_> {
    fn schema_id(&self) -> &SaveSchemaId {
        &self.schema_id
    }

    fn current_version(&self) -> u32 {
        self.current_version
    }

    fn migrate(&self, from_version: u32, mut value: Value) -> Result<Value> {
        let mut version = from_version;
        while version < self.current_version {
            let step = self.step_from(version)?;
            value = step.migrate(value).map_err(|error| {
                error.at_field(format!(
                    "migration_{}_to_{}",
                    step.source_version(),
                    step.target_version()
                ))
            })?;
            version = step.target_version();
        }
        if version == self.current_version {
            Ok(value)
        } else {
            Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                format!(
                    "save migration chain overshot version {version}; expected {}",
                    self.current_version
                ),
            ))
        }
    }
}

fn validate_migration_steps(
    schema_id: &SaveSchemaId,
    current_version: u32,
    steps: &[&dyn SaveMigrationStep],
) -> Result<()> {
    steps.iter().try_for_each(|step| {
        if step.schema_id() != schema_id {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                format!(
                    "save migration step schema id `{}` does not match expected `{}`",
                    step.schema_id().as_str(),
                    schema_id.as_str()
                ),
            ));
        }
        if step.source_version() >= step.target_version() {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                format!(
                    "save migration step must advance versions: {} -> {}",
                    step.source_version(),
                    step.target_version()
                ),
            ));
        }
        if step.target_version() > current_version {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                format!(
                    "save migration step target {} exceeds current version {}",
                    step.target_version(),
                    current_version
                ),
            ));
        }
        if steps
            .iter()
            .filter(|other| other.source_version() == step.source_version())
            .count()
            > 1
        {
            return Err(DataError::new(
                DataErrorKind::DuplicateField,
                format!(
                    "duplicate save migration step from version {}",
                    step.source_version()
                ),
            ));
        }
        Ok(())
    })
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
    expected_schema_id: &SaveSchemaId,
    current_schema_version: u32,
    registry: &CodecRegistry,
    options: &SaveDecodeOptions,
    migration: Option<&dyn SaveMigration>,
) -> Result<Value> {
    let envelope = SaveEnvelope::decode_bytes(input, options)?;
    if &envelope.schema_id != expected_schema_id {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "save schema id `{}` does not match expected `{}`",
                envelope.schema_id.as_str(),
                expected_schema_id.as_str()
            ),
        ));
    }
    if envelope.schema_version > current_schema_version {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            format!(
                "save schema version {} is newer than supported {}",
                envelope.schema_version, current_schema_version
            ),
        ));
    }
    let codec = registry.by_id(&envelope.codec_id)?;
    let value = codec.decode_value(&envelope.payload, shape, &options.codec)?;
    let value = match envelope.schema_version.cmp(&current_schema_version) {
        std::cmp::Ordering::Equal => value,
        std::cmp::Ordering::Less => {
            let Some(migration) = migration else {
                return Err(DataError::new(
                    DataErrorKind::InvalidEncoding,
                    format!(
                        "save schema version {} requires migration to {}",
                        envelope.schema_version, current_schema_version
                    ),
                ));
            };
            if migration.schema_id() != expected_schema_id {
                return Err(DataError::new(
                    DataErrorKind::InvalidEncoding,
                    format!(
                        "save migration schema id `{}` does not match expected `{}`",
                        migration.schema_id().as_str(),
                        expected_schema_id.as_str()
                    ),
                ));
            }
            if migration.current_version() != current_schema_version {
                return Err(DataError::new(
                    DataErrorKind::InvalidEncoding,
                    format!(
                        "save migration current version {} does not match expected {}",
                        migration.current_version(),
                        current_schema_version
                    ),
                ));
            }
            migration.migrate(envelope.schema_version, value)?
        }
        std::cmp::Ordering::Greater => unreachable!("future version rejected before decode"),
    };
    encode_with_shape(&value, shape)?;
    Ok(value)
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

    fn bounded_len(&mut self, label: &str, max: u64) -> Result<usize> {
        let len = u64::from(self.u32()?);
        if len > max {
            return Err(DataError::limit(format!(
                "save {label} length {len} exceeds {max}"
            )));
        }
        usize::try_from(len).map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                format!("save {label} length does not fit usize"),
            )
        })
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

    fn finish(&self, allow_trailing_data: bool) -> Result<()> {
        if allow_trailing_data || self.offset == self.input.len() {
            Ok(())
        } else {
            Err(DataError::new(
                DataErrorKind::TrailingData,
                "save envelope has trailing data",
            ))
        }
    }
}
